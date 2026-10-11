//! V2 managed Agent inventory roots backed by STO authenticated trees.
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

const PROJECTION_KIND: &[u8] = b"tos.cmd.managed-agent-inventory-projection-v2";
const PROFILE_KIND: &[u8] = b"tos.cmd.managed-agent-source-profile-v2";
const INVENTORY_MAGIC: &[u8] = b"TOS-AI-V2\0";
const MIXED_PROJECTION_KIND: &[u8] = b"tos.cmd.managed-mixed-source-inventory-projection-v1";
const MIXED_PROFILE_KIND: &[u8] = b"tos.cmd.managed-mixed-source-profile-v1";
const MIXED_AGENT_PROFILE_KIND: &[u8] = b"tos.cmd.managed-mixed-agent-source-profile-v1";
const MIXED_INVENTORY_MAGIC: &[u8] = b"TOS-MI-V1\0";
const MIXED_SCHEMA_VERSION: &str = "tos_managed_mixed_source_inventory_member_v1";
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

#[derive(Clone, Debug)]
pub(super) struct AddressedInventoryV2 {
    projections: AuthenticatedTreeDescriptorV2,
    profiles: AuthenticatedTreeDescriptorV2,
}

impl AddressedInventoryV2 {
    /// Build both trees during the explicit cold V2 transition. This is the
    /// only full projection traversal; warm successors use `apply_changed`.
    pub(super) fn build(
        tx: &mut Transaction<'_>,
        store: &SegmentStore,
        domain: &str,
        generation: u64,
        limits: AuthenticatedTreeLimitsV1,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> DurableResult<Self> {
        let mut work = AuthenticatedTreeWorkV1::default();
        Self::build_with_work(
            tx, store, domain, generation, limits, deadline, cancelled, &mut work,
        )
    }

    /// Build while adding successful STO operation work to the caller's
    /// existing counters. STO exposes work only on success; partial work for
    /// a failed STO call is unavailable and is not estimated here.
    pub(super) fn build_with_work(
        tx: &mut Transaction<'_>,
        store: &SegmentStore,
        domain: &str,
        generation: u64,
        limits: AuthenticatedTreeLimitsV1,
        deadline: Instant,
        cancelled: &AtomicBool,
        work: &mut AuthenticatedTreeWorkV1,
    ) -> DurableResult<Self> {
        active(deadline, cancelled)?;
        let admission = format!(
            "WITH members AS ({RETAINED_PROJECTIONS}) SELECT count(*),coalesce(sum(octet_length(inventory_projection)),0),count(*) FILTER(WHERE inventory_projection IS NULL) FROM members"
        );
        let sequence = as_i64(generation)?;
        let admitted = tx.query_one(&admission, &[&domain, &sequence])?;
        let admitted_count = as_u64(admitted.get(0))?;
        if admitted_count > limits.max_rows
            || as_u64(admitted.get(1))? > MAX_PROJECTION_BYTES
            || admitted.get::<_, i64>(2) != 0
        {
            return Err(DurableError::Refused(
                "Agent projection exceeds the cold inventory envelope or is incomplete",
            ));
        }

        let sql = format!(
            "WITH members AS ({RETAINED_PROJECTIONS}) SELECT * FROM members ORDER BY subject COLLATE \"C\""
        );
        let mut rows = tx.query_raw(
            &sql,
            [&domain as &(dyn postgres::types::ToSql + Sync), &sequence],
        )?;
        let mut profile_rows = BTreeMap::<String, Digest256>::new();
        let mut cursor = String::new();
        let mut total_key_bytes = 0usize;
        let mut count = 0u64;
        let mut cursor_error = None;
        let mut stopped = false;
        let entries = std::iter::from_fn(|| {
            if stopped {
                return None;
            }
            match rows.next() {
                Ok(Some(row)) => {
                    let entry = (|| -> DurableResult<AuthenticatedTreeEntryV1> {
                        active(deadline, cancelled)?;
                        let subject: String = row.get("subject");
                        if subject <= cursor || !subject.starts_with("ToS/") {
                            return Err(DurableError::Corrupt(
                                "Agent projection path coverage/order differs",
                            ));
                        }
                        let path = RelativePath::parse(&subject).map_err(|_| {
                            DurableError::Corrupt("Agent projection path is invalid")
                        })?;
                        let key = path.as_str().as_bytes().to_vec();
                        if key.len() > limits.max_key_bytes {
                            return Err(DurableError::Refused(
                                "Agent projection key exceeds authenticated tree bound",
                            ));
                        }
                        total_key_bytes = total_key_bytes
                            .checked_add(key.len())
                            .filter(|n| *n <= MAX_TOTAL_MEMBERSHIP_KEY_BYTES)
                            .ok_or(DurableError::Refused(
                                "Agent projection keys exceed authenticated tree envelope",
                            ))?;

                        let value = projection_value(&row)?;
                        let raw: Vec<u8> = row
                            .get::<_, Option<Vec<u8>>>("inventory_projection")
                            .ok_or(DurableError::Refused(
                                "managed inventory projection incomplete; FullOnly required",
                            ))?;
                        collect_profiles(&value, &mut profile_rows)?;
                        count = count
                            .checked_add(1)
                            .ok_or(DurableError::Corrupt("Agent projection coverage overflow"))?;
                        cursor = subject;
                        Ok(AuthenticatedTreeEntryV1 {
                            key,
                            value: Digest256::of_bytes(&raw).as_bytes().to_vec(),
                        })
                    })();
                    match entry {
                        Ok(entry) => Some(Ok(entry)),
                        Err(error) => {
                            stopped = true;
                            Some(Err(tree_input_error(error)))
                        }
                    }
                }
                Ok(None) => None,
                Err(error) => {
                    stopped = true;
                    cursor_error = Some(error);
                    Some(Err(SegmentError::new(
                        SegmentErrorCode::CorruptBytes,
                        "Agent projection database cursor failed",
                    )))
                }
            }
        });
        let projection_tree_result = store.build_authenticated_tree_v2_with_work(
            PROJECTION_KIND,
            entries,
            limits,
            deadline,
            cancelled,
        );
        drop(rows);
        if let Some(error) = cursor_error {
            return Err(DurableError::Database(error));
        }
        let (projection_tree, projection_work) = projection_tree_result.map_err(tree_error)?;
        add_tree_work(work, projection_work)?;
        if count != admitted_count {
            return Err(DurableError::Corrupt(
                "Agent projection coverage count differs",
            ));
        }
        let (profile_tree, profile_work) = store
            .build_authenticated_tree_v2_with_work(
                PROFILE_KIND,
                profile_rows.into_iter().map(|(path, digest)| {
                    Ok(AuthenticatedTreeEntryV1 {
                        key: path.into_bytes(),
                        value: digest.as_bytes().to_vec(),
                    })
                }),
                limits,
                deadline,
                cancelled,
            )
            .map_err(tree_error)?;
        add_tree_work(work, profile_work)?;
        let result = Self {
            projections: projection_tree,
            profiles: profile_tree,
        };
        result.require_store(store)?;
        if result.projections.entries != count || result.profiles.entries > MAX_PROFILE_COUNT {
            return Err(DurableError::Corrupt(
                "Agent authenticated inventory tree coverage differs",
            ));
        }
        Ok(result)
    }

    /// Consume a complete, semantically sealed cold assessment cursor. The
    /// caller owns source verification; this builder grants only STO custody.
    /// Unlike the compatibility PG traversal, it retains one bounded canonical
    /// projection at a time and does not impose a whole projection byte cap.
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

    /// Streamed cold assessment variant. The callback is checked by the STO
    /// tree writer before each irreversible node operation; it is supplied by
    /// the protected owner route and does not grant source admission.
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
                "streamed inventory owner fence changed",
            ));
        }
        if expected_count > limits.max_rows {
            return Err(DurableError::Refused("streamed inventory row bound"));
        }
        let mut projections = projections.into_iter();
        let mut profiles = BTreeMap::<String, Digest256>::new();
        let mut after = None::<String>;
        let mut count = 0u64;
        let mut failure = None;
        let mut ended = false;
        let entries = std::iter::from_fn(|| {
            if ended {
                return None;
            }
            let input = match projections.next() {
                Some(input) => input,
                None => {
                    ended = true;
                    return None;
                }
            };
            let entry = (|| -> DurableResult<AuthenticatedTreeEntryV1> {
                active(deadline, cancelled)?;
                let (path, raw) = input?;
                if !path.starts_with("ToS/")
                    || after.as_ref().is_some_and(|prior| path <= *prior)
                    || path.len() > limits.max_key_bytes
                {
                    return Err(DurableError::Corrupt("streamed inventory path/order"));
                }
                let value = parse_projection(&path, &raw, None)?;
                collect_profiles(&value, &mut profiles)?;
                count = count
                    .checked_add(1)
                    .filter(|n| *n <= expected_count)
                    .ok_or(DurableError::Corrupt("streamed inventory extra rows"))?;
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
                        "streamed inventory assessment cursor failed",
                    )))
                }
            }
        });
        let state_bytes = tree_callback_state_bytes(limits)?;
        let result = store.build_authenticated_tree_v2_with_work_and_io_and_state_and_callback(
            PROJECTION_KIND,
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
        let (projection_tree, projection_work) = result.map_err(tree_error)?;
        add_tree_work(work, projection_work)?;
        // A partial producer is never a complete selected inventory witness.
        if !ended || count != expected_count || projection_tree.entries != count {
            return Err(DurableError::Corrupt(
                "streamed inventory EOF/count differs",
            ));
        }
        if !owner_check() {
            return Err(DurableError::Conflict(
                "streamed inventory owner fence changed",
            ));
        }
        let (profile_tree, profile_work) = store
            .build_authenticated_tree_v2_with_work_and_io_and_state_and_callback(
                PROFILE_KIND,
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
        let result = Self {
            projections: projection_tree,
            profiles: profile_tree,
        };
        result.require_store(store)?;
        result.check_descriptor_pair()?;
        active(deadline, cancelled)?;
        Ok(result)
    }

    /// Apply only newly inserted current projections. Existing subjects cannot
    /// be rewritten on this controlled creation path. Profile conflicts fail
    /// before either immutable tree is advanced.
    pub(super) fn apply_changed(
        &self,
        store: &SegmentStore,
        changed: &ProjectionRows,
        limits: AuthenticatedTreeLimitsV1,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> DurableResult<Self> {
        let mut work = AuthenticatedTreeWorkV1::default();
        self.apply_changed_with_work(store, changed, limits, deadline, cancelled, &mut work)
    }

    /// Apply the addressed inserts while adding successful lookup and
    /// copy-on-write STO work to the caller's existing counters. Failed STO
    /// calls provide no partial work value, so this reports only work returned
    /// by preceding successful calls.
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
        if changed.len() > MAX_MEMBERS {
            return Err(DurableError::Refused(
                "Agent projection delta exceeds member bound",
            ));
        }
        if changed.is_empty() {
            return Ok(self.clone());
        }
        let next_count = self
            .projections
            .entries
            .checked_add(changed.len() as u64)
            .ok_or(DurableError::Refused(
                "Agent projection tree entry count overflow",
            ))?;
        let mut projection_delta = Vec::with_capacity(changed.len());
        let mut profile_delta = BTreeMap::<String, Digest256>::new();
        let mut total_key_bytes = 0usize;
        let mut total_projection_bytes = 0usize;

        for (subject, raw) in changed {
            active(deadline, cancelled)?;
            let path = RelativePath::parse(subject)
                .map_err(|_| DurableError::Corrupt("Agent delta projection path"))?;
            if !path.as_str().starts_with("ToS/") || raw.len() > 1_048_576 {
                return Err(DurableError::Refused(
                    "Agent delta projection exceeds its member envelope",
                ));
            }
            total_key_bytes = total_key_bytes
                .checked_add(path.as_str().len())
                .filter(|n| *n <= MAX_TOTAL_MEMBERSHIP_KEY_BYTES)
                .ok_or(DurableError::Refused(
                    "Agent delta keys exceed authenticated tree envelope",
                ))?;
            total_projection_bytes = total_projection_bytes
                .checked_add(raw.len())
                .filter(|n| *n <= MAX_PROJECTION_BYTES as usize)
                .ok_or(DurableError::Refused(
                    "Agent delta projections exceed cold metadata envelope",
                ))?;
            if path.as_str().as_bytes().len() > limits.max_key_bytes {
                return Err(DurableError::Refused(
                    "Agent delta key exceeds authenticated tree bound",
                ));
            }
            let (existing, lookup_work) = store
                .lookup_authenticated_tree_v2_with_work(
                    &self.projections,
                    path.as_str().as_bytes(),
                    limits,
                    deadline,
                    cancelled,
                )
                .map_err(tree_error)?;
            add_tree_work(work, lookup_work)?;
            if existing.is_some() {
                return Err(DurableError::Conflict(
                    "Agent delta rewrites an addressed projection",
                ));
            }
            let value = parse_projection(subject, raw, None)?;
            collect_profiles(&value, &mut profile_delta)?;
            projection_delta.push(AuthenticatedTreeDeltaV1 {
                key: path.as_str().as_bytes().to_vec(),
                value: Some(Digest256::of_bytes(raw).as_bytes().to_vec()),
            });
        }

        let mut profile_changes = Vec::new();
        for (path, digest) in profile_delta {
            let (existing, lookup_work) = store
                .lookup_authenticated_tree_v2_with_work(
                    &self.profiles,
                    path.as_bytes(),
                    limits,
                    deadline,
                    cancelled,
                )
                .map_err(tree_error)?;
            add_tree_work(work, lookup_work)?;
            if let Some(existing) = existing {
                if existing.as_slice() != digest.as_bytes() {
                    return Err(DurableError::Corrupt(
                        "Agent source profile contributions disagree",
                    ));
                }
            } else {
                profile_changes.push(AuthenticatedTreeDeltaV1 {
                    key: path.into_bytes(),
                    value: Some(digest.as_bytes().to_vec()),
                });
            }
        }
        let next_profile_count = self
            .profiles
            .entries
            .checked_add(profile_changes.len() as u64)
            .ok_or(DurableError::Refused(
                "Agent source profile tree entry count overflow",
            ))?;
        if next_profile_count > MAX_PROFILE_COUNT {
            return Err(DurableError::Refused(
                "Agent source profile union exceeds selected file bound",
            ));
        }
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
        let profile_tree = if profile_changes.is_empty() {
            self.profiles.clone()
        } else {
            let (profile_tree, profile_work) = store
                .apply_authenticated_tree_delta_v2_with_work(
                    &self.profiles,
                    profile_changes.into_iter().map(Ok),
                    limits,
                    deadline,
                    cancelled,
                )
                .map_err(tree_error)?;
            add_tree_work(work, profile_work)?;
            profile_tree
        };
        if projection_tree.entries != next_count || profile_tree.entries > MAX_PROFILE_COUNT {
            return Err(DurableError::Corrupt(
                "Agent authenticated inventory delta coverage differs",
            ));
        }
        let result = Self {
            projections: projection_tree,
            profiles: profile_tree,
        };
        result.require_store(store)?;
        Ok(result)
    }

    /// Bind the V2 root to both authenticated namespaces. The explicit tag
    /// and version keep this value distinct from the legacy V1 transcript.
    pub(super) fn root(&self) -> Digest256 {
        let mut hash = Digest256Hasher::new();
        part(&mut hash, b"tos-managed-agent-inventory-root-v2");
        part(&mut hash, &2u16.to_be_bytes());
        part(&mut hash, PROJECTION_KIND);
        part(&mut hash, self.projections.commitment.as_bytes());
        part(&mut hash, &self.projections.entries.to_be_bytes());
        part(&mut hash, PROFILE_KIND);
        part(&mut hash, self.profiles.commitment.as_bytes());
        part(&mut hash, &self.profiles.entries.to_be_bytes());
        hash.finalize()
    }

    /// Exact selected projection commitment; this is not a catalog epoch.
    pub(super) fn projection_digest(
        &self,
        store: &SegmentStore,
        path: &RelativePath,
        limits: AuthenticatedTreeLimitsV1,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> DurableResult<Option<Digest256>> {
        let mut work = AuthenticatedTreeWorkV1::default();
        self.projection_digest_with_work(store, path, limits, deadline, cancelled, &mut work)
    }

    /// Look up one exact projection while adding its measured STO path work to
    /// the caller's existing counters. STO exposes work only on success.
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
            .map_err(|_| DurableError::Corrupt("Agent selected projection digest length"))?;
        Ok(Some(Digest256::from_bytes(digest)))
    }

    pub(super) fn checked_projection(
        subject: &str,
        raw: &[u8],
        content: &str,
    ) -> DurableResult<tos_foundation::JsonValue> {
        parse_projection(subject, raw, Some(content))
    }

    pub(super) fn projection_entries(&self) -> u64 {
        self.projections.entries
    }

    /// Resolve the bounded profile tree against the exact selected source
    /// bytes, then bind the canonical Agent schema/configuration template.
    /// Work is bounded by selected source files, never by cohort member count.
    pub(super) fn select(
        &self,
        store: &SegmentStore,
        ctx: &CommandContext,
        limits: AuthenticatedTreeLimitsV1,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> DurableResult<ManagedAgentInventory> {
        self.require_store(store)?;
        if self.profiles.entries > MAX_PROFILE_COUNT {
            return Err(DurableError::Refused(
                "Agent source profile union exceeds selected file bound",
            ));
        }
        let mut stream = store
            .stream_authenticated_tree_v2(&self.profiles, limits)
            .map_err(tree_error)?;
        let mut profiles = BTreeMap::<String, Digest256>::new();
        let mut previous = String::new();
        while let Some(entry) = stream.next_row(deadline, cancelled).map_err(tree_error)? {
            active(deadline, cancelled)?;
            let path = String::from_utf8(entry.key)
                .map_err(|_| DurableError::Corrupt("Agent source profile tree key"))?;
            if path <= previous {
                return Err(DurableError::Corrupt(
                    "Agent source profile tree ordering differs",
                ));
            }
            RelativePath::parse(&path)
                .map_err(|_| DurableError::Corrupt("Agent source profile tree path"))?;
            let bytes: [u8; 32] = entry
                .value
                .try_into()
                .map_err(|_| DurableError::Corrupt("Agent source profile tree digest"))?;
            profiles.insert(path.clone(), Digest256::from_bytes(bytes));
            previous = path;
        }
        if profiles.len() as u64 != self.profiles.entries {
            return Err(DurableError::Corrupt(
                "Agent source profile tree coverage differs",
            ));
        }

        let mut profile_values = Vec::with_capacity(profiles.len());
        for (path, digest) in &profiles {
            active(deadline, cancelled)?;
            let relative = RelativePath::parse(path)
                .map_err(|_| DurableError::Corrupt("Agent profile path"))?;
            let raw = ctx
                .file(&relative)
                .map_err(source_error)?
                .ok_or(DurableError::Conflict(
                    "Agent profile resource not selected",
                ))?;
            if Digest256::of_bytes(raw) != *digest {
                return Err(DurableError::Conflict(
                    "Agent profile resource digest differs",
                ));
            }
            profile_values.push((path.as_str(), cmd::string(&digest.to_hex())));
        }
        let profile_map = cmd::object(profile_values);
        let template = crate::source_creation::agent_dependency_template(ctx, profile_map)
            .map_err(source_error)?;
        let template_bytes = cmd::canonical(&template).map_err(source_error)?;
        let root = self.root();
        let mut dependency_hash = Digest256Hasher::new();
        part(
            &mut dependency_hash,
            b"tos-managed-agent-inventory-dependencies-v2",
        );
        part(&mut dependency_hash, &2u16.to_be_bytes());
        part(&mut dependency_hash, PROJECTION_KIND);
        part(&mut dependency_hash, root.as_bytes());
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
        Ok(ManagedAgentInventory {
            commitment_version: 2,
            root,
            dependencies: dependency_hash.finalize().to_prefixed(),
        })
    }

    /// Require both authenticated namespaces to describe projections freshly
    /// derived from the complete selected source. This is the cold-reopen
    /// counterpart to `build_from_assessed_projections`: it reads the retained
    /// descriptor and checks exact path/digest coverage without replacing a
    /// still-published root.
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
                "addressed source projection coverage differs",
            ));
        }

        let mut expected_profiles = BTreeMap::new();
        for (path, raw) in projections {
            let value = parse_projection(path, raw, None)?;
            collect_profiles(&value, &mut expected_profiles)?;
        }

        let mut projection_rows = store
            .stream_authenticated_tree_v2(&self.projections, limits)
            .map_err(tree_error)?;
        let mut expected = projections.iter();
        let mut previous = String::new();
        let mut count = 0u64;
        while let Some(entry) = projection_rows
            .next_row(deadline, cancelled)
            .map_err(tree_error)?
        {
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
        if count != self.projections.entries || expected.next().is_some() {
            return Err(DurableError::Corrupt(
                "addressed source projection EOF differs",
            ));
        }

        let mut profile_rows = store
            .stream_authenticated_tree_v2(&self.profiles, limits)
            .map_err(tree_error)?;
        let mut expected = expected_profiles.iter();
        let mut previous = String::new();
        let mut count = 0u64;
        while let Some(entry) = profile_rows
            .next_row(deadline, cancelled)
            .map_err(tree_error)?
        {
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
        if count != self.profiles.entries || expected.next().is_some() {
            return Err(DurableError::Corrupt(
                "addressed source profile EOF differs",
            ));
        }

        // The source bytes must also still provide the exact selected profile
        // files and dependency template bound by this authenticated inventory.
        self.select(store, ctx, limits, deadline, cancelled)?;
        Ok(())
    }

    pub(super) fn encode(&self) -> DurableResult<Vec<u8>> {
        self.check_descriptor_pair()?;
        let projections = self
            .projections
            .encode(MAX_INVENTORY_DESCRIPTOR_BYTES)
            .map_err(tree_error)?;
        let profiles = self
            .profiles
            .encode(MAX_INVENTORY_DESCRIPTOR_BYTES)
            .map_err(tree_error)?;
        let total = INVENTORY_MAGIC
            .len()
            .checked_add(4)
            .and_then(|n| n.checked_add(projections.len()))
            .and_then(|n| n.checked_add(4))
            .and_then(|n| n.checked_add(profiles.len()))
            .filter(|n| *n <= MAX_INVENTORY_DESCRIPTOR_BYTES)
            .ok_or(DurableError::Refused(
                "Agent inventory descriptor exceeds byte bound",
            ))?;
        let mut output = Vec::with_capacity(total);
        output.extend_from_slice(INVENTORY_MAGIC);
        append_bytes(&mut output, &projections)?;
        append_bytes(&mut output, &profiles)?;
        Ok(output)
    }

    pub(super) fn decode(raw: &[u8]) -> DurableResult<Self> {
        if raw.len() > MAX_INVENTORY_DESCRIPTOR_BYTES || !raw.starts_with(INVENTORY_MAGIC) {
            return Err(DurableError::Corrupt(
                "Agent inventory descriptor version/size",
            ));
        }
        let mut offset = INVENTORY_MAGIC.len();
        let projection_bytes = take_bytes(raw, &mut offset)?;
        let profile_bytes = take_bytes(raw, &mut offset)?;
        if offset != raw.len() {
            return Err(DurableError::Corrupt(
                "Agent inventory descriptor trailing bytes",
            ));
        }
        let result = Self {
            projections: AuthenticatedTreeDescriptorV2::decode(
                projection_bytes,
                MAX_INVENTORY_DESCRIPTOR_BYTES,
            )
            .map_err(tree_error)?,
            profiles: AuthenticatedTreeDescriptorV2::decode(
                profile_bytes,
                MAX_INVENTORY_DESCRIPTOR_BYTES,
            )
            .map_err(tree_error)?,
        };
        result.check_descriptor_pair()?;
        if result.encode()? != raw {
            return Err(DurableError::Corrupt(
                "Agent inventory descriptor noncanonical encoding",
            ));
        }
        Ok(result)
    }

    pub(super) fn require_store(&self, store: &SegmentStore) -> DurableResult<()> {
        self.check_descriptor_pair()?;
        if self.projections.store_id != store.store_id()
            || self.profiles.store_id != store.store_id()
            || self.projections.domain_digest != store.domain_digest()
            || self.profiles.domain_digest != store.domain_digest()
        {
            return Err(DurableError::Conflict(
                "Agent inventory tree custody identity differs",
            ));
        }
        Ok(())
    }

    fn check_descriptor_pair(&self) -> DurableResult<()> {
        if self.projections.kind != PROJECTION_KIND
            || self.profiles.kind != PROFILE_KIND
            || self.projections.store_id != self.profiles.store_id
            || self.projections.domain_digest != self.profiles.domain_digest
            || self.profiles.entries > MAX_PROFILE_COUNT
            || self.projections.root.as_ref().map(|root| root.entries)
                != (self.projections.entries != 0).then_some(self.projections.entries)
            || self.profiles.root.as_ref().map(|root| root.entries)
                != (self.profiles.entries != 0).then_some(self.profiles.entries)
        {
            return Err(DurableError::Corrupt(
                "Agent inventory tree descriptors disagree",
            ));
        }
        Ok(())
    }
}

/// Mixed source inventory custody. Its first two trees describe the complete
/// mixed projection/profile surface; the third tree contains only the common
/// Agent registry baseline and schemas contributed by Agent metadata/forms.
/// This is a distinct descriptor from `AddressedInventoryV2`: an old
/// Agent-only descriptor is never interpreted as a mixed inventory.
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

    /// Apply initial-only added source members. Existing projection keys are
    /// refused; ordinary revisions use their owner-specific retained-projection
    /// route and are not a supported replacement operation here.
    pub(super) fn apply_changed(
        &self,
        store: &SegmentStore,
        changed: &ProjectionRows,
        limits: AuthenticatedTreeLimitsV1,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> DurableResult<Self> {
        let mut work = AuthenticatedTreeWorkV1::default();
        self.apply_changed_with_work(store, changed, limits, deadline, cancelled, &mut work)
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

    pub(super) fn projection_digest(
        &self,
        store: &SegmentStore,
        path: &RelativePath,
        limits: AuthenticatedTreeLimitsV1,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> DurableResult<Option<Digest256>> {
        let mut work = AuthenticatedTreeWorkV1::default();
        self.projection_digest_with_work(store, path, limits, deadline, cancelled, &mut work)
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
        .find(|(kind, _)| kind.as_str() == "agent")
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
        &[
            "schema_version",
            "path",
            "raw_sha256",
            "records",
            "source_profiles",
            "events",
            "anchors",
            "form",
        ],
    )
    .map_err(source_error)?;
    let raw_digest = cmd::text(&value, "raw_sha256").map_err(source_error)?;
    let parsed_digest = Digest256::from_hex(raw_digest)
        .map_err(|_| DurableError::Corrupt("mixed projection source digest"))?;
    let records = cmd::field(&value, "records")
        .map_err(source_error)?
        .as_object()
        .ok_or(DurableError::Corrupt("mixed projection records map"))?;
    for kind in crate::source_claims::NATIVE_CATALOG_KINDS {
        if !records.iter().any(|(key, _)| key.as_str() == *kind) {
            return Err(DurableError::Corrupt(
                "mixed projection native kind coverage",
            ));
        }
    }
    if records.iter().any(|(_, rows)| rows.as_array().is_none()) {
        return Err(DurableError::Corrupt("mixed projection record rows"));
    }
    if let Some(form) = value.object_get("form") {
        if form.as_object().is_some() {
            cmd::exact_keys(form, &["raw_sha256", "form_ids"]).map_err(source_error)?;
        } else if form != &tos_foundation::JsonValue::Null {
            return Err(DurableError::Corrupt("mixed projection form shape"));
        }
    }
    if cmd::text(&value, "schema_version").map_err(source_error)? != MIXED_SCHEMA_VERSION
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

fn parse_projection(
    subject: &str,
    raw: &[u8],
    expected_content_digest: Option<&str>,
) -> DurableResult<tos_foundation::JsonValue> {
    if raw.len() > 1_048_576 {
        return Err(DurableError::Refused(
            "Agent projection exceeds cold metadata bound",
        ));
    }
    let value = cmd::parse(raw).map_err(source_error)?;
    cmd::exact_keys(
        &value,
        &[
            "schema_version",
            "path",
            "raw_sha256",
            "records",
            "source_profiles",
            "events",
            "anchors",
            "form",
        ],
    )
    .map_err(source_error)?;
    let raw_digest = cmd::text(&value, "raw_sha256").map_err(source_error)?;
    let parsed_digest = Digest256::from_hex(raw_digest)
        .map_err(|_| DurableError::Corrupt("Agent projection source digest"))?;
    if cmd::text(&value, "schema_version").map_err(source_error)?
        != "tos_managed_agent_inventory_member_v1"
        || cmd::text(&value, "path").map_err(source_error)? != subject
        || expected_content_digest.is_some_and(|expected| expected != parsed_digest.to_hex())
        || cmd::canonical(&value).map_err(source_error)? != raw
    {
        return Err(DurableError::Corrupt(
            "Agent projection raw/classification binding differs",
        ));
    }
    RelativePath::parse(subject)
        .map_err(|_| DurableError::Corrupt("Agent projection path is invalid"))?;
    Ok(value)
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

fn tree_input_error(error: DurableError) -> SegmentError {
    match error {
        DurableError::Storage(error) => error,
        DurableError::Refused("source operation deadline or cancellation") => SegmentError::new(
            SegmentErrorCode::DeadlineExceeded,
            "Agent inventory projection read deadline or cancellation",
        ),
        _ => SegmentError::new(
            SegmentErrorCode::CorruptBytes,
            "Agent inventory projection input failed validation",
        ),
    }
}
