//! Separate owner-private package persistence for profile, Claim, Text,
//! Alignment and Assessment.
//!
//! This namespace is deliberately absent from public source selection and
//! projection. Only sealed crate-owned family capabilities may ask the
//! coordinator to commit bytes; public consumers receive neither table
//! access nor private package rows.

use super::{DurableError, DurablePgCoordinator, DurableResult, as_i64, as_u64, parse_hex, part};
use postgres::{Client, GenericClient};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};
use tos_foundation::{Digest256, Digest256Hasher};

pub(crate) const PRIVATE_SCHEMA_PROFILE: &str =
    super::durable_compatibility::PRIVATE_PACKAGE_ABI_PROFILE_V4;
const MAX_TARGET_REF_BYTES: usize = 4096;
const MAX_MEMBER_NAME_BYTES: usize = 128;
const MAX_MEMBERS: usize = 64;
const MAX_MEMBER_BYTES: usize = 12 * 1024 * 1024;
const MAX_TEXT_PACKAGE_BYTES: usize = 12 * 1024 * 1024;
const MAX_ALIGNMENT_PACKAGE_BYTES: usize = 12 * 1024 * 1024;
const MAX_PROFILE_CLAIM_PACKAGE_BYTES: usize = 8 * 1024 * 1024;
const MAX_ASSESSMENT_PACKAGE_BYTES: usize = 2 * 1024 * 1024;
const MAX_ASSESSMENT_MEMBER_BYTES: usize = 1 * 1024 * 1024;
const MAX_ARCHIVE_BYTES: usize = 16 * 1024 * 1024;
const MAX_ARCHIVES_PER_COMMIT: usize = 1;
const MAX_RECEIPT_BYTES: usize = 1024 * 1024;
const MAX_IDENTITY_ROWS: usize = 32_768;
const MAX_IDENTITY_FILES: usize = 2_048;
const MAX_IDENTITY_BYTES: usize = 64 * 1024 * 1024;
pub(crate) const MAX_PACKAGE_HISTORY_ROWS: usize = 1_024;
pub(crate) const MAX_PACKAGE_HISTORY_BYTES: usize = 40 * 1024 * 1024;
pub(crate) const MAX_PACKAGE_HISTORY_TARGETS: usize = 65;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PrivateOwnerPackageFamilyV1 {
    Text,
    Alignment,
    Profile,
    Claim,
    Assessment,
}

impl PrivateOwnerPackageFamilyV1 {
    fn as_str(self) -> &'static str {
        match self {
            Self::Text => "text",
            Self::Alignment => "alignment",
            Self::Profile => "profile",
            Self::Claim => "claim",
            Self::Assessment => "assessment",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PrivateOwnerScopeV1 {
    pub(crate) domain: String,
    pub(crate) owner_scope: String,
    pub(crate) context_binding: Digest256,
}

impl PrivateOwnerScopeV1 {
    /// Stable opaque identity derived from the exact private owner context.
    /// No account name, home path, or credential is persisted as a key.
    pub(crate) fn for_context(domain: &str, context_binding: Digest256) -> DurableResult<Self> {
        if domain.is_empty() || domain.len() > 256 {
            return Err(DurableError::Invalid("private owner domain identity"));
        }
        let mut hasher = Digest256Hasher::new();
        part(&mut hasher, b"tos-private-owner-scope-v1");
        part(&mut hasher, domain.as_bytes());
        part(&mut hasher, context_binding.as_bytes());
        Ok(Self {
            domain: domain.to_owned(),
            owner_scope: hasher.finalize().to_hex(),
            context_binding,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PrivateOwnerSourceBindingV1 {
    pub(crate) head_seq: u64,
    pub(crate) source_epoch: u64,
    pub(crate) source_generation: u64,
    pub(crate) source_revision: Digest256,
    pub(crate) source_audit_generation: u64,
    pub(crate) rights_version: u64,
    pub(crate) rule_version: u64,
    pub(crate) contract_digest: Digest256,
    pub(crate) job_id: String,
    pub(crate) job_fence: u64,
    pub(crate) readset_digest: Digest256,
    pub(crate) dependency_digest: Digest256,
    /// The private owner inventory generation observed by all package and
    /// identity reads used by the semantic owner preparation.
    pub(crate) private_owner_audit_generation: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PrivateOwnerPackageSnapshotV1 {
    pub(crate) scope: PrivateOwnerScopeV1,
    pub(crate) target_ref: String,
    pub(crate) family: PrivateOwnerPackageFamilyV1,
    pub(crate) revision: u64,
    pub(crate) package_digest: Digest256,
    pub(crate) members: BTreeMap<String, Vec<u8>>,
    pub(crate) text_identity_root: Option<Digest256>,
    pub(crate) text_identity_count: Option<usize>,
    pub(crate) private_audit_generation: u64,
}

/// Package metadata for owner operations that need existence and identity but
/// must not hydrate private member bytes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PrivateOwnerPackageHeaderV1 {
    pub(crate) scope: PrivateOwnerScopeV1,
    pub(crate) target_ref: String,
    pub(crate) current: Option<PrivateOwnerPackageCurrentHeaderV1>,
    pub(crate) private_audit_generation: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PrivateOwnerPackageCurrentHeaderV1 {
    pub(crate) family: PrivateOwnerPackageFamilyV1,
    pub(crate) revision: u64,
    pub(crate) package_digest: Digest256,
    pub(crate) member_count: usize,
    pub(crate) total_bytes: u64,
    pub(crate) command_id: String,
    pub(crate) commit_seq: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PrivateOwnerSelectedMemberV1 {
    pub(crate) target_ref: String,
    pub(crate) member_name: String,
    pub(crate) family: PrivateOwnerPackageFamilyV1,
    pub(crate) revision: u64,
    pub(crate) package_digest: Digest256,
    pub(crate) member_digest: Digest256,
    pub(crate) bytes: Vec<u8>,
    pub(crate) private_audit_generation: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PrivateOwnerCommandReceiptMetadataV1 {
    pub(crate) command_id: String,
    pub(crate) target_digest: Digest256,
    pub(crate) target_matches: bool,
    pub(crate) family: PrivateOwnerPackageFamilyV1,
    pub(crate) request_digest: Digest256,
    pub(crate) raw_request_digest: Digest256,
    pub(crate) owner_context_binding: Digest256,
    pub(crate) owner_configuration_digest: Digest256,
    pub(crate) package_digest: Digest256,
    pub(crate) response_digest: Digest256,
    pub(crate) commit_seq: u64,
    pub(crate) private_audit_generation_after: u64,
    /// Private generation observed by the original command's source binding.
    pub(crate) source_private_generation: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PrivateOwnerCommandMetadataV1 {
    pub(crate) header: PrivateOwnerPackageHeaderV1,
    /// Present for any receipt with this ID in the owner scope. `target_digest`
    /// lets the caller distinguish a receipt for another target without
    /// disclosing that target's path.
    pub(crate) receipt: Option<PrivateOwnerCommandReceiptMetadataV1>,
}

/// A bounded private read view held under one RR snapshot and the owner-scope
/// shared advisory lock. The session lock is acquired before this snapshot.
/// Callbacks may open a second coordinator for public source reads, but must
/// not attempt a private write in this same scope.
pub(crate) struct PrivateOwnerReadSession<'connection> {
    transaction: Option<postgres::Transaction<'connection>>,
    scope: PrivateOwnerScopeV1,
    private_audit_generation: u64,
    current_headers: BTreeMap<String, Option<PrivateOwnerPackageCurrentHeaderV1>>,
}

/// Session-scoped shared advisory lock acquired before the RR transaction.
/// Taking an xact advisory lock as the first statement of an RR transaction
/// can establish the MVCC snapshot before a lock wait completes. This guard
/// keeps the lock across the subsequent transaction and releases it on every
/// ordinary return path and during unwinding.
struct PrivateOwnerSharedReadLock<'client> {
    client: &'client mut Client,
    lock_a: i32,
    lock_b: i32,
    held: bool,
}

impl<'client> PrivateOwnerSharedReadLock<'client> {
    fn acquire(
        client: &'client mut Client,
        scope: &PrivateOwnerScopeV1,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> DurableResult<Self> {
        let (lock_a, lock_b) = private_scope_advisory_keys(scope);
        let mut delay = Duration::from_millis(5);
        const MAX_DELAY: Duration = Duration::from_millis(50);
        loop {
            check_private_history_deadline(deadline, cancelled)?;
            let acquired: bool = client
                .query_one(
                    "SELECT pg_try_advisory_lock_shared($1,$2)",
                    &[&lock_a, &lock_b],
                )?
                .get(0);
            if acquired {
                return Ok(PrivateOwnerSharedReadLock {
                    client,
                    lock_a,
                    lock_b,
                    held: true,
                });
            }
            check_private_history_deadline(deadline, cancelled)?;
            let remaining = deadline.saturating_duration_since(Instant::now());
            std::thread::sleep(delay.min(MAX_DELAY).min(remaining));
            delay = (delay * 2).min(MAX_DELAY);
        }
    }

    fn release(mut self) -> DurableResult<()> {
        let released: bool = self
            .client
            .query_one(
                "SELECT pg_advisory_unlock_shared($1,$2)",
                &[&self.lock_a, &self.lock_b],
            )?
            .get(0);
        self.held = false;
        if released {
            Ok(())
        } else {
            Err(DurableError::Corrupt(
                "private owner shared advisory lock was not held",
            ))
        }
    }
}

impl Drop for PrivateOwnerSharedReadLock<'_> {
    fn drop(&mut self) {
        if self.held {
            let _ = self.client.query_one(
                "SELECT pg_advisory_unlock_shared($1,$2)",
                &[&self.lock_a, &self.lock_b],
            );
            self.held = false;
        }
    }
}

fn with_private_owner_shared_read_lock<T>(
    client: &mut Client,
    scope: &PrivateOwnerScopeV1,
    deadline: Instant,
    cancelled: &AtomicBool,
    operation: impl FnOnce(&mut Client) -> DurableResult<T>,
) -> DurableResult<T> {
    let lock = PrivateOwnerSharedReadLock::acquire(client, scope, deadline, cancelled)?;
    let result = check_private_history_deadline(deadline, cancelled)
        .and_then(|()| operation(&mut *lock.client));
    let released = lock.release();
    match released {
        Ok(()) => result,
        Err(error) => Err(error),
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PrivateTextIdentityMatchV1 {
    pub(crate) token: String,
    pub(crate) target_ref: String,
    pub(crate) member_name: String,
    pub(crate) member_digest: Digest256,
    pub(crate) revision: u64,
    pub(crate) package_digest: Digest256,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PrivateTextIdentityLookupV1 {
    pub(crate) matches: Vec<PrivateTextIdentityMatchV1>,
    pub(crate) private_audit_generation: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PrivateOwnerPackageArchiveV1 {
    pub(crate) reference: String,
    pub(crate) source_revision: Digest256,
    pub(crate) members: BTreeMap<String, Vec<u8>>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PrivateOwnerArchiveSnapshotV1 {
    pub(crate) scope: PrivateOwnerScopeV1,
    pub(crate) target_ref: String,
    pub(crate) reference: String,
    pub(crate) family: PrivateOwnerPackageFamilyV1,
    pub(crate) source_revision: Digest256,
    pub(crate) package_digest: Digest256,
    pub(crate) members: BTreeMap<String, Vec<u8>>,
    pub(crate) private_audit_generation: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PrivateOwnerPackageWriteV1 {
    pub(crate) scope: PrivateOwnerScopeV1,
    pub(crate) family: PrivateOwnerPackageFamilyV1,
    pub(crate) target_ref: String,
    pub(crate) expected_current: Option<PrivateOwnerPackageSnapshotV1>,
    pub(crate) members: BTreeMap<String, Vec<u8>>,
    pub(crate) archives: Vec<PrivateOwnerPackageArchiveV1>,
    pub(crate) command_id: String,
    pub(crate) request_digest: Digest256,
    pub(crate) raw_request_digest: Digest256,
    pub(crate) owner_configuration_digest: Digest256,
    pub(crate) source_binding: PrivateOwnerSourceBindingV1,
    pub(crate) response_bytes: Vec<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PrivateOwnerPackageReplayV1 {
    /// Whether this call resolved to an already committed command receipt.
    /// A fresh append returns false; a sequential or raced existing receipt
    /// returns true without inserting a second package or receipt.
    pub(crate) replayed: bool,
    pub(crate) commit_seq: u64,
    /// Revision of this target's committed package, independent of the
    /// owner-wide command sequence. Replays retain the historical revision.
    pub(crate) package_revision: u64,
    pub(crate) package_digest: Digest256,
    pub(crate) private_audit_generation_after: u64,
    pub(crate) response_bytes: Vec<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PrivateOwnerPackageHistoryEntryV1 {
    pub(crate) revision: u64,
    pub(crate) family: PrivateOwnerPackageFamilyV1,
    pub(crate) package_digest: Digest256,
    pub(crate) previous_digest: Option<Digest256>,
    pub(crate) members: BTreeMap<String, Vec<u8>>,
    pub(crate) command_id: String,
    pub(crate) request_digest: Digest256,
    pub(crate) raw_request_digest: Digest256,
    pub(crate) owner_configuration_digest: Digest256,
    pub(crate) source_binding: PrivateOwnerSourceBindingV1,
    pub(crate) commit_seq: u64,
    pub(crate) private_audit_generation_after: u64,
    pub(crate) response_bytes: Vec<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PrivateOwnerPackageHistoryV1 {
    pub(crate) scope: PrivateOwnerScopeV1,
    pub(crate) target_ref: String,
    pub(crate) family: Option<PrivateOwnerPackageFamilyV1>,
    pub(crate) current: Option<PrivateOwnerPackageSnapshotV1>,
    pub(crate) history: Vec<PrivateOwnerPackageHistoryEntryV1>,
    pub(crate) private_audit_generation: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PrivateOwnerIdentityInputsV1 {
    pub(crate) files: BTreeMap<String, Vec<u8>>,
    pub(crate) visited_entries: usize,
    pub(crate) private_audit_generation: u64,
}

/// Deliberately crate-private seal: external crates cannot authorize private
/// commits. Implementations are limited to the Text, Profile, Claim and
/// Assessment owner adapters, each retaining its typed semantic preparation
/// guard.
pub(crate) mod capability_seal {
    pub(crate) trait Sealed {}
}

pub(crate) trait PrivateOwnerPackageCapabilityV1: capability_seal::Sealed {
    fn write(&self) -> &PrivateOwnerPackageWriteV1;
    /// Optional exact current-head closure for owner operations whose result
    /// depends on several immutable histories. The broker checks it under the
    /// same exclusive scope lock as the package append.
    fn expected_history_heads(&self) -> Option<&BTreeMap<String, Option<Vec<u8>>>> {
        None
    }
    fn revalidate(
        &mut self,
        current: Option<&PrivateOwnerPackageSnapshotV1>,
        replay: Option<&PrivateOwnerPackageReplayV1>,
    ) -> DurableResult<()>;
}

pub(crate) fn private_owner_dependency_digest(
    reads: &[(String, Vec<u8>)],
) -> DurableResult<Digest256> {
    let mut ordered = reads.iter().collect::<Vec<_>>();
    ordered.sort_by(|a, b| a.0.cmp(&b.0));
    if ordered.windows(2).any(|w| w[0].0 == w[1].0) {
        return Err(DurableError::Invalid(
            "private owner dependency paths are duplicated",
        ));
    }
    let mut hasher = Digest256Hasher::new();
    part(&mut hasher, b"tos-private-owner-dependencies-v1");
    part(
        &mut hasher,
        &u64::try_from(ordered.len())
            .map_err(|_| DurableError::Invalid("private dependency count"))?
            .to_be_bytes(),
    );
    for (path, bytes) in ordered {
        if path.is_empty() || path.len() > MAX_TARGET_REF_BYTES || path.contains('\0') {
            return Err(DurableError::Invalid("private dependency path"));
        }
        let digest = Digest256::of_bytes(bytes);
        part(&mut hasher, path.as_bytes());
        part(
            &mut hasher,
            &u64::try_from(bytes.len())
                .map_err(|_| DurableError::Invalid("private dependency length"))?
                .to_be_bytes(),
        );
        part(&mut hasher, digest.as_bytes());
    }
    Ok(hasher.finalize())
}

/// Hash the existing canonical `SourceReads` JSON and its addressed member
/// observations. This is audit evidence for the exact PG read epoch, not a
/// complete source-corpus proof.
pub(crate) fn private_owner_readset_digest(
    readset: &crate::durable_adapter::source_cohort::revision::SourceRevisionReadSet,
) -> DurableResult<Digest256> {
    use crate::PredicateRead;
    let reads = readset.reads();
    let values = reads
        .reads
        .iter()
        .map(|read| match read {
            PredicateRead::Exact {
                namespace,
                key,
                expected_version,
                expected_digest,
            } => serde_json::json!([
                "exact",
                namespace,
                key,
                expected_version,
                expected_digest.map(|d| d.to_hex()),
                reads.locators.get(key).map(|d| d.to_hex())
            ]),
            PredicateRead::Absent { namespace, key } => {
                serde_json::json!(["absent", namespace, key])
            }
            PredicateRead::Generation {
                predicate,
                observed_generation,
            } => serde_json::json!([
                "generation",
                predicate.kind.as_str(),
                predicate.owner,
                predicate.scope,
                predicate.token,
                predicate.definition_version,
                observed_generation
            ]),
        })
        .collect::<Vec<_>>();
    let value = match (&reads.managed_original, &reads.revision) {
        (None, None) => serde_json::Value::Array(values),
        (Some(original), None) => serde_json::json!({
            "profile":"tos.managed-agent-original-reads-v1",
            "reads":values,
            "original":original
        }),
        (None, Some(revision)) => serde_json::json!({
            "profile":"tos.pg-family-tagged-source-revision-reads-v1",
            "family":revision.family_tag,
            "source_head_sequence":revision.source_head_sequence,
            "predecessor_source_revision":revision.predecessor_source_revision.0.to_hex(),
            "reads":values
        }),
        (Some(_), Some(_)) => {
            return Err(DurableError::Invalid(
                "private source readset combines incompatible profiles",
            ));
        }
    };
    let observations = readset
        .observations()
        .iter()
        .map(|(path, observation)| {
            serde_json::json!([
                path,
                observation.locator.to_hex(),
                observation.member.custody_revision,
                observation.member.current_generation,
                observation.member.commit_seq,
                observation.member.raw.len(),
                Digest256::of_bytes(&observation.member.raw).to_hex()
            ])
        })
        .collect::<Vec<_>>();
    let canonical = serde_json::to_vec(&serde_json::json!({
        "reads": value,
        "observations": observations
    }))
    .map_err(|_| DurableError::Invalid("private source readset encoding"))?;
    let mut hasher = Digest256Hasher::new();
    part(&mut hasher, b"tos-private-owner-source-readset-v1");
    part(&mut hasher, &(canonical.len() as u64).to_be_bytes());
    part(&mut hasher, &canonical);
    Ok(hasher.finalize())
}

pub(crate) fn package_digest(members: &BTreeMap<String, Vec<u8>>) -> DurableResult<Digest256> {
    let bindings = members
        .iter()
        .map(|(name, raw)| (name, Digest256::of_bytes(raw).to_hex()))
        .collect::<BTreeMap<_, _>>();
    let encoded = serde_json::to_vec(&bindings)
        .map_err(|_| DurableError::Invalid("private package digest encoding"))?;
    Ok(Digest256::of_bytes(&encoded))
}

fn package_digest_from_member_digests(
    members: &BTreeMap<String, String>,
) -> DurableResult<Digest256> {
    let encoded = serde_json::to_vec(members)
        .map_err(|_| DurableError::Corrupt("private package member digest encoding"))?;
    Ok(Digest256::of_bytes(&encoded))
}

fn validate_members(
    family: PrivateOwnerPackageFamilyV1,
    members: &BTreeMap<String, Vec<u8>>,
    archive: bool,
) -> DurableResult<(Digest256, u64)> {
    if archive && family == PrivateOwnerPackageFamilyV1::Assessment {
        return Err(DurableError::Invalid(
            "assessment packages do not contain archives",
        ));
    }
    let max_count = MAX_MEMBERS + usize::from(archive);
    if members.is_empty() || members.len() > max_count {
        return Err(DurableError::Invalid("private package member count"));
    }
    let mut total = 0usize;
    for (name, bytes) in members {
        if name.is_empty()
            || name == "."
            || name == ".."
            || name.len() > MAX_MEMBER_NAME_BYTES
            || name.contains('/')
            || name.contains('\\')
            || name.contains('\0')
            || bytes.len() > MAX_MEMBER_BYTES
        {
            return Err(DurableError::Invalid(
                "private package member identity/budget",
            ));
        }
        total = total
            .checked_add(bytes.len())
            .ok_or(DurableError::Invalid("private package size overflow"))?;
    }
    if family == PrivateOwnerPackageFamilyV1::Assessment
        && (members.len() != 2
            || !members.contains_key("head")
            || !members.contains_key("batch.json")
            || members
                .values()
                .any(|bytes| bytes.len() > MAX_ASSESSMENT_MEMBER_BYTES))
    {
        return Err(DurableError::Invalid(
            "assessment package must contain exact bounded head and batch members",
        ));
    }
    let max_total = if archive {
        MAX_ARCHIVE_BYTES
    } else {
        match family {
            PrivateOwnerPackageFamilyV1::Text => MAX_TEXT_PACKAGE_BYTES,
            PrivateOwnerPackageFamilyV1::Alignment => MAX_ALIGNMENT_PACKAGE_BYTES,
            PrivateOwnerPackageFamilyV1::Profile | PrivateOwnerPackageFamilyV1::Claim => {
                MAX_PROFILE_CLAIM_PACKAGE_BYTES
            }
            PrivateOwnerPackageFamilyV1::Assessment => MAX_ASSESSMENT_PACKAGE_BYTES,
        }
    };
    if total == 0 || total > max_total {
        return Err(DurableError::Invalid("private package byte budget"));
    }
    Ok((package_digest(members)?, total as u64))
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct PrivateTextIdentityContribution {
    token: String,
    member_name: String,
    member_digest: Digest256,
}

fn private_text_identity_contributions(
    family: PrivateOwnerPackageFamilyV1,
    target_ref: &str,
    members: &BTreeMap<String, Vec<u8>>,
) -> DurableResult<(Digest256, Vec<PrivateTextIdentityContribution>)> {
    if !matches!(
        family,
        PrivateOwnerPackageFamilyV1::Text | PrivateOwnerPackageFamilyV1::Alignment
    ) {
        return Err(DurableError::Invalid(
            "private owner identity package family",
        ));
    }
    if !target_ref.starts_with("ToS/") || tos_foundation::RelativePath::parse(target_ref).is_err() {
        return Err(DurableError::Invalid("private Text identity package route"));
    }
    let mut contributions = Vec::new();
    for (member_name, bytes) in members {
        let candidate = match family {
            PrivateOwnerPackageFamilyV1::Text => {
                crate::source_text_identity::is_candidate(member_name)
            }
            PrivateOwnerPackageFamilyV1::Alignment => {
                crate::source_text_identity::is_alignment_candidate(member_name)
            }
            _ => false,
        };
        if !candidate {
            continue;
        }
        let logical_path = format!("{target_ref}/{member_name}");
        if logical_path.len() > MAX_TARGET_REF_BYTES
            || tos_foundation::RelativePath::parse(&logical_path).is_err()
        {
            return Err(DurableError::Invalid("private Text identity member route"));
        }
        let tokens = crate::source_text_identity::text_identity_ids_for_member(member_name, bytes)
            .map_err(DurableError::Source)?;
        let member_digest = Digest256::of_bytes(bytes);
        for token in tokens {
            if token.is_empty() || token.len() > MAX_TARGET_REF_BYTES || token.contains('\0') {
                return Err(DurableError::Invalid("private Text identity token"));
            }
            contributions.push(PrivateTextIdentityContribution {
                token,
                member_name: member_name.clone(),
                member_digest,
            });
            if contributions.len() > MAX_IDENTITY_ROWS {
                return Err(DurableError::Source(
                    crate::source_command::SourceCommandError::Unsupported(
                        "private Text identity row budget",
                    ),
                ));
            }
        }
    }
    contributions.sort();
    if contributions.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err(DurableError::Corrupt(
            "duplicate private Text identity contribution",
        ));
    }
    let root = private_text_identity_root(target_ref, &contributions)?;
    Ok((root, contributions))
}

fn private_text_identity_root(
    target_ref: &str,
    contributions: &[PrivateTextIdentityContribution],
) -> DurableResult<Digest256> {
    if contributions.len() > MAX_IDENTITY_ROWS
        || contributions.windows(2).any(|pair| pair[0] >= pair[1])
    {
        return Err(DurableError::Corrupt(
            "private Text identity contribution ordering",
        ));
    }
    let mut hasher = Digest256Hasher::new();
    part(&mut hasher, b"tos-private-text-identity-current-v1");
    part(&mut hasher, target_ref.as_bytes());
    part(
        &mut hasher,
        &u64::try_from(contributions.len())
            .map_err(|_| DurableError::Invalid("private Text identity count"))?
            .to_be_bytes(),
    );
    for contribution in contributions {
        part(&mut hasher, contribution.token.as_bytes());
        part(&mut hasher, contribution.member_name.as_bytes());
        part(&mut hasher, contribution.member_digest.as_bytes());
    }
    Ok(hasher.finalize())
}

fn validate_private_text_identity_snapshot(
    family: PrivateOwnerPackageFamilyV1,
    target_ref: &str,
    members: &BTreeMap<String, Vec<u8>>,
    stored_root: Option<Digest256>,
    stored_count: Option<usize>,
) -> DurableResult<(Option<Digest256>, Option<usize>)> {
    if matches!(
        family,
        PrivateOwnerPackageFamilyV1::Text | PrivateOwnerPackageFamilyV1::Alignment
    ) {
        let (root, contributions) =
            private_text_identity_contributions(family, target_ref, members)?;
        if stored_root.is_some_and(|value| value != root)
            || stored_count.is_some_and(|value| value != contributions.len())
        {
            return Err(DurableError::Corrupt(
                "private Text identity root or count differs from package",
            ));
        }
        Ok((Some(root), Some(contributions.len())))
    } else if stored_root.is_some() || stored_count.is_some() {
        Err(DurableError::Corrupt(
            "non-Text private package carries an identity index",
        ))
    } else {
        Ok((None, None))
    }
}

fn validate_scope(scope: &PrivateOwnerScopeV1) -> DurableResult<()> {
    if scope.domain.is_empty()
        || scope.domain.len() > 256
        || scope.owner_scope.len() != 64
        || !scope
            .owner_scope
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        return Err(DurableError::Invalid("private owner scope identity"));
    }
    let expected = SelfScope::derive(&scope.domain, scope.context_binding);
    if expected != scope.owner_scope {
        return Err(DurableError::Invalid("private owner scope binding"));
    }
    Ok(())
}

struct SelfScope;
impl SelfScope {
    fn derive(domain: &str, context_binding: Digest256) -> String {
        let mut hasher = Digest256Hasher::new();
        part(&mut hasher, b"tos-private-owner-scope-v1");
        part(&mut hasher, domain.as_bytes());
        part(&mut hasher, context_binding.as_bytes());
        hasher.finalize().to_hex()
    }
}

fn validate_target_ref(reference: &str) -> DurableResult<()> {
    if reference.is_empty() || reference.len() > MAX_TARGET_REF_BYTES || reference.contains('\0') {
        return Err(DurableError::Invalid("private package target reference"));
    }
    Ok(())
}

fn require_private_schema(client: &mut impl GenericClient) -> DurableResult<()> {
    // PostgreSQL truncates identifiers to 63 bytes; use the installed catalog spelling.
    let compatible: bool = client
        .query_one(
            "WITH required_tables(name) AS (VALUES
               ('cmd2_private_owner_scope'),('cmd2_private_schema_profile'),
               ('cmd2_private_owner_audit_fence'),('cmd2_private_package_current'),
               ('cmd2_private_package_history'),('cmd2_private_package_member'),
               ('cmd2_private_package_archive'),('cmd2_private_package_archive_member'),
               ('cmd2_private_command_receipt'),('cmd2_private_log'),('cmd2_private_outbox'),
               ('cmd2_private_text_identity')),
             required_indexes(name) AS (VALUES
               ('cmd2_private_package_member_name_seek'),
               ('cmd2_private_package_member_digest_key'),
               ('cmd2_private_package_current_text_identity_key'),
               ('cmd2_private_text_identity_point')),
             required_columns(table_name,column_name) AS (VALUES
               ('cmd2_private_package_current','text_identity_root'),
               ('cmd2_private_package_current','text_identity_count')),
             required_constraints(name) AS (VALUES
               ('cmd2_private_package_history_family_check'),
               ('cmd2_private_package_history_assessment_shape_check'),
               ('cmd2_private_package_history_alignment_size_check'),
               ('cmd2_private_package_current_family_check'),
               ('cmd2_private_package_current_assessment_shape_check'),
               ('cmd2_private_package_current_alignment_size_check'),
               ('cmd2_private_package_current_text_identity_root_check'),
               ('cmd2_private_package_current_text_identity_count_check'),
               ('cmd2_private_package_current_text_identity_shape_check'),
               ('cmd2_private_text_identity_family_check'),
               ('cmd2_private_package_archive_family_check'),
               ('cmd2_private_command_receipt_family_check'),
               ('cmd2_private_log_family_check'),
               ('cmd2_private_outbox_family_check')),
             required_functions(name) AS (VALUES
               ('cmd2_private_bump_audit_fence'),('cmd2_private_refuse_immutable_mutation'),
               ('cmd2_private_refuse_truncate'),('cmd2_private_guard_scope_update'),
               ('cmd2_private_guard_audit_update')),
             required_triggers(name) AS (VALUES
               ('cmd2_private_scope_binding_update'),('cmd2_private_scope_binding_delete'),
               ('cmd2_private_audit_fence_update'),('cmd2_private_audit_fence_immutable'),
               ('cmd2_private_audit_cmd2_private_package_current'),
               ('cmd2_private_audit_cmd2_private_package_history'),
               ('cmd2_private_audit_cmd2_private_package_member'),
               ('cmd2_private_audit_cmd2_private_package_archive'),
               ('cmd2_private_audit_cmd2_private_package_archive_member'),
               ('cmd2_private_audit_cmd2_private_command_receipt'),
               ('cmd2_private_audit_cmd2_private_log'),
               ('cmd2_private_audit_cmd2_private_outbox'),
               ('cmd2_private_audit_cmd2_private_text_identity'),
               ('cmd2_private_schema_profile_immutable'),
               ('cmd2_private_schema_profile_truncate'),
               ('cmd2_private_history_immutable'),('cmd2_private_member_immutable'),
               ('cmd2_private_archive_immutable'),('cmd2_private_archive_member_immutable'),
               ('cmd2_private_receipt_immutable'),('cmd2_private_log_immutable'),
               ('cmd2_private_outbox_immutable'),
               ('cmd2_private_refuse_cmd2_private_owner_scope_truncate'),
               ('cmd2_private_refuse_cmd2_private_owner_audit_fence_truncate'),
               ('cmd2_private_refuse_cmd2_private_package_current_truncate'),
               ('cmd2_private_refuse_cmd2_private_package_history_truncate'),
               ('cmd2_private_refuse_cmd2_private_package_member_truncate'),
               ('cmd2_private_refuse_cmd2_private_package_archive_truncate'),
               ('cmd2_private_refuse_cmd2_private_package_archive_member_truncat'),
               ('cmd2_private_refuse_cmd2_private_command_receipt_truncate'),
               ('cmd2_private_refuse_cmd2_private_log_truncate'),
               ('cmd2_private_refuse_cmd2_private_outbox_truncate'),
               ('cmd2_private_refuse_cmd2_private_text_identity_truncate'))
             SELECT
               (SELECT count(*)=12 FROM required_tables r JOIN pg_class c
                 ON c.relname=r.name AND c.relkind IN ('r','p')
                JOIN pg_namespace n ON n.oid=c.relnamespace
                WHERE n.nspname=current_schema())
               AND (SELECT count(*)=4 FROM required_indexes r JOIN pg_class c
                 ON c.relname=r.name AND c.relkind='i'
                JOIN pg_namespace n ON n.oid=c.relnamespace
                WHERE n.nspname=current_schema())
               AND (SELECT count(*)=2 FROM required_columns r
                 JOIN information_schema.columns c
                   ON c.table_schema=current_schema()
                  AND c.table_name=r.table_name AND c.column_name=r.column_name)
               AND (SELECT count(*)=14 FROM required_constraints r JOIN pg_constraint c
                 ON c.conname=r.name JOIN pg_class t ON t.oid=c.conrelid
                JOIN pg_namespace n ON n.oid=t.relnamespace
                WHERE n.nspname=current_schema())
               AND (SELECT count(*)=5 FROM required_functions r JOIN pg_proc p
                 ON p.proname=r.name JOIN pg_namespace n ON n.oid=p.pronamespace
                WHERE n.nspname=current_schema())
               AND (SELECT count(*)=33 FROM required_triggers r JOIN pg_trigger t
                 ON t.tgname=r.name AND NOT t.tgisinternal
                JOIN pg_class c ON c.oid=t.tgrelid
                JOIN pg_namespace n ON n.oid=c.relnamespace
                WHERE n.nspname=current_schema())",
            &[],
        )?
        .get(0);
    if !compatible {
        return Err(DurableError::Refused(
            "private owner package schema is absent or incomplete",
        ));
    }
    let profiles = client.query(
        "SELECT profile_id FROM cmd2_private_schema_profile WHERE singleton",
        &[],
    )?;
    if profiles.len() != 1 || profiles[0].get::<_, String>(0) != PRIVATE_SCHEMA_PROFILE {
        return Err(DurableError::Refused(
            "private owner package schema profile is incompatible",
        ));
    }
    Ok(())
}

pub(crate) fn require_compatible_private_package_schema(
    client: &mut impl GenericClient,
) -> DurableResult<()> {
    require_private_schema(client)
}

fn private_audit_generation(
    client: &mut impl GenericClient,
    scope: &PrivateOwnerScopeV1,
) -> DurableResult<u64> {
    let value = client.query_opt(
        "SELECT generation FROM cmd2_private_owner_audit_fence WHERE domain=$1 AND owner_scope=$2",
        &[&scope.domain, &scope.owner_scope],
    )?;
    value.map_or(Ok(0), |row| as_u64(row.get(0)))
}

fn private_scope_advisory_keys(scope: &PrivateOwnerScopeV1) -> (i32, i32) {
    let mut hasher = Digest256Hasher::new();
    part(&mut hasher, b"tos-private-owner-scope-lock-v1");
    part(&mut hasher, scope.domain.as_bytes());
    part(&mut hasher, scope.owner_scope.as_bytes());
    let digest = hasher.finalize();
    let bytes = digest.as_bytes();
    (
        i32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]),
        i32::from_be_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]),
    )
}

fn verify_context_binding(
    client: &mut impl GenericClient,
    scope: &PrivateOwnerScopeV1,
) -> DurableResult<bool> {
    let row = client.query_opt(
        "SELECT context_binding FROM cmd2_private_owner_scope WHERE domain=$1 AND owner_scope=$2",
        &[&scope.domain, &scope.owner_scope],
    )?;
    Ok(row.is_none_or(|row| row.get::<_, String>(0) == scope.context_binding.to_hex()))
}

fn private_family_max_bytes(family: PrivateOwnerPackageFamilyV1) -> usize {
    match family {
        PrivateOwnerPackageFamilyV1::Text => MAX_TEXT_PACKAGE_BYTES,
        PrivateOwnerPackageFamilyV1::Alignment => MAX_ALIGNMENT_PACKAGE_BYTES,
        PrivateOwnerPackageFamilyV1::Profile | PrivateOwnerPackageFamilyV1::Claim => {
            MAX_PROFILE_CLAIM_PACKAGE_BYTES
        }
        PrivateOwnerPackageFamilyV1::Assessment => MAX_ASSESSMENT_PACKAGE_BYTES,
    }
}

fn private_family_member_max_bytes(family: PrivateOwnerPackageFamilyV1) -> usize {
    if family == PrivateOwnerPackageFamilyV1::Assessment {
        MAX_ASSESSMENT_MEMBER_BYTES
    } else {
        MAX_MEMBER_BYTES
    }
}

fn verify_current_member_manifest(
    client: &mut impl GenericClient,
    scope: &PrivateOwnerScopeV1,
    target_ref: &str,
    revision: u64,
    expected_count: usize,
    expected_total: u64,
    expected_digest: Digest256,
    family: PrivateOwnerPackageFamilyV1,
) -> DurableResult<()> {
    let rows = client.query(
        "SELECT member_name,member_digest,octet_length(member_bytes)
         FROM cmd2_private_package_member
         WHERE domain=$1 AND owner_scope=$2 AND target_ref=$3 AND revision=$4
         ORDER BY member_name COLLATE \"C\" LIMIT 66",
        &[
            &scope.domain,
            &scope.owner_scope,
            &target_ref,
            &as_i64(revision)?,
        ],
    )?;
    if rows.len() != expected_count {
        return Err(DurableError::Corrupt(
            "private package current member metadata count",
        ));
    }
    let mut member_digests = BTreeMap::new();
    let mut total = 0u64;
    for row in rows {
        let name: String = row.get(0);
        let digest: String = row.get(1);
        let size: i32 = row.get(2);
        parse_hex(digest.clone())?;
        if name.is_empty()
            || name == "."
            || name == ".."
            || name.len() > MAX_MEMBER_NAME_BYTES
            || name.contains('/')
            || name.contains('\\')
            || name.contains('\0')
            || size < 0
            || size as usize > private_family_member_max_bytes(family)
            || member_digests.insert(name, digest).is_some()
        {
            return Err(DurableError::Corrupt(
                "private package current member metadata shape",
            ));
        }
        total = total
            .checked_add(size as u64)
            .ok_or(DurableError::Corrupt("private package total byte overflow"))?;
    }
    if total != expected_total
        || package_digest_from_member_digests(&member_digests)? != expected_digest
    {
        return Err(DurableError::Corrupt(
            "private package current member metadata digest or size",
        ));
    }
    Ok(())
}

fn read_current_package_header(
    client: &mut impl GenericClient,
    scope: &PrivateOwnerScopeV1,
    target_ref: &str,
    private_generation: u64,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> DurableResult<Option<PrivateOwnerPackageCurrentHeaderV1>> {
    check_private_history_deadline(deadline, cancelled)?;
    let row = client.query_opt(
        "SELECT c.family,c.revision,c.package_digest,c.member_count,c.total_bytes,
                c.command_id,c.commit_seq,c.text_identity_root,c.text_identity_count,
                h.target_ref,h.family,h.revision,h.package_digest,h.member_count,h.total_bytes,
                h.command_id,h.commit_seq,h.owner_context_binding,h.owner_configuration_digest,
                h.request_digest,h.raw_request_digest,h.private_owner_audit_generation,
                r.command_id,r.target_digest,r.family,r.request_digest,r.raw_request_digest,
                r.owner_context_binding,r.owner_configuration_digest,r.package_digest,
                r.commit_seq,r.private_audit_generation_after
         FROM cmd2_private_package_current c
         LEFT JOIN cmd2_private_package_history h
           ON h.domain=c.domain AND h.owner_scope=c.owner_scope
          AND h.target_ref=c.target_ref AND h.revision=c.revision
         LEFT JOIN cmd2_private_command_receipt r
           ON r.domain=c.domain AND r.owner_scope=c.owner_scope
          AND r.command_id=c.command_id
         WHERE c.domain=$1 AND c.owner_scope=$2 AND c.target_ref=$3",
        &[&scope.domain, &scope.owner_scope, &target_ref],
    )?;
    check_private_history_deadline(deadline, cancelled)?;
    let Some(row) = row else {
        let has_history: bool = client
            .query_one(
                "SELECT EXISTS(SELECT 1 FROM cmd2_private_package_history
             WHERE domain=$1 AND owner_scope=$2 AND target_ref=$3)",
                &[&scope.domain, &scope.owner_scope, &target_ref],
            )?
            .get(0);
        check_private_history_deadline(deadline, cancelled)?;
        if has_history {
            return Err(DurableError::Corrupt(
                "private package history exists without a current header",
            ));
        }
        return Ok(None);
    };

    macro_rules! required {
        ($index:expr, $type:ty, $reason:expr) => {
            row.get::<_, Option<$type>>($index)
                .ok_or(DurableError::Corrupt($reason))?
        };
    }

    let family = parse_family(row.get::<_, String>(0))?;
    let revision = as_u64(row.get(1))?;
    let package_digest = parse_hex(row.get::<_, String>(2))?;
    let member_count: i32 = row.get(3);
    let total_bytes = as_u64(row.get(4))?;
    let command_id: String = row.get(5);
    let commit_seq = as_u64(row.get(6))?;
    let stored_identity_root = row.get::<_, Option<String>>(7);
    let stored_identity_count = row.get::<_, Option<i32>>(8);
    if member_count <= 0
        || member_count as usize > MAX_MEMBERS
        || revision == 0
        || total_bytes == 0
        || total_bytes > private_family_max_bytes(family) as u64
        || command_id.is_empty()
        || commit_seq == 0
    {
        return Err(DurableError::Corrupt(
            "private package current header shape",
        ));
    }
    match family {
        PrivateOwnerPackageFamilyV1::Text | PrivateOwnerPackageFamilyV1::Alignment => {
            let root = stored_identity_root.ok_or(DurableError::Corrupt(
                "private package identity root absent",
            ))?;
            parse_hex(root)?;
            let count = stored_identity_count.ok_or(DurableError::Corrupt(
                "private package identity count absent",
            ))?;
            if count < 0 || count as usize > MAX_IDENTITY_ROWS {
                return Err(DurableError::Corrupt("private package identity count"));
            }
        }
        _ if stored_identity_root.is_some() || stored_identity_count.is_some() => {
            return Err(DurableError::Corrupt(
                "non-identity private family has an identity marker",
            ));
        }
        _ => {}
    }

    let history_target: String = required!(9, String, "private package current history absent");
    let history_family = parse_family(required!(
        10,
        String,
        "private package current history absent"
    ))?;
    let history_revision = as_u64(required!(11, i64, "private package current history absent"))?;
    let history_digest = parse_hex(required!(
        12,
        String,
        "private package current history absent"
    ))?;
    let history_member_count = required!(13, i32, "private package current history absent");
    let history_total_bytes = as_u64(required!(14, i64, "private package current history absent"))?;
    let history_command_id = required!(15, String, "private package current history absent");
    let history_commit_seq = as_u64(required!(16, i64, "private package current history absent"))?;
    let history_context = required!(17, String, "private package current history absent");
    let history_configuration = required!(18, String, "private package current history absent");
    let history_request = required!(19, String, "private package current history absent");
    let history_raw_request = required!(20, String, "private package current history absent");
    let source_private_generation =
        as_u64(required!(21, i64, "private package current history absent"))?;
    let receipt_command_id = required!(22, String, "private package current receipt absent");
    let receipt_target_digest = parse_hex(required!(
        23,
        String,
        "private package current receipt absent"
    ))?;
    let receipt_family = parse_family(required!(
        24,
        String,
        "private package current receipt absent"
    ))?;
    let receipt_request = required!(25, String, "private package current receipt absent");
    let receipt_raw_request = required!(26, String, "private package current receipt absent");
    let receipt_context = required!(27, String, "private package current receipt absent");
    let receipt_configuration = required!(28, String, "private package current receipt absent");
    let receipt_digest = parse_hex(required!(
        29,
        String,
        "private package current receipt absent"
    ))?;
    let receipt_commit_seq = as_u64(required!(30, i64, "private package current receipt absent"))?;
    let generation_after = as_u64(required!(31, i64, "private package current receipt absent"))?;
    let exact_target_digest = Digest256::of_bytes(target_ref.as_bytes());
    if history_target != target_ref
        || history_family != family
        || history_revision != revision
        || history_digest != package_digest
        || history_member_count != member_count
        || history_total_bytes != total_bytes
        || history_command_id != command_id
        || history_commit_seq != commit_seq
        || history_context != scope.context_binding.to_hex()
        || receipt_command_id != command_id
        || receipt_target_digest != exact_target_digest
        || receipt_family != family
        || receipt_request != history_request
        || receipt_raw_request != history_raw_request
        || receipt_context != history_context
        || receipt_configuration != history_configuration
        || receipt_digest != package_digest
        || receipt_commit_seq != commit_seq
        || source_private_generation >= generation_after
        || generation_after > private_generation
    {
        return Err(DurableError::Corrupt(
            "private package current header receipt binding",
        ));
    }
    parse_hex(history_configuration)?;
    parse_hex(history_request)?;
    parse_hex(history_raw_request)?;
    verify_current_member_manifest(
        client,
        scope,
        target_ref,
        revision,
        member_count as usize,
        total_bytes,
        package_digest,
        family,
    )?;
    check_private_history_deadline(deadline, cancelled)?;
    Ok(Some(PrivateOwnerPackageCurrentHeaderV1 {
        family,
        revision,
        package_digest,
        member_count: member_count as usize,
        total_bytes,
        command_id,
        commit_seq,
    }))
}

fn read_private_command_receipt_metadata(
    client: &mut impl GenericClient,
    scope: &PrivateOwnerScopeV1,
    target_ref: &str,
    command_id: &str,
    private_generation: u64,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> DurableResult<Option<PrivateOwnerCommandReceiptMetadataV1>> {
    Ok(read_private_command_receipt_record(
        client,
        scope,
        target_ref,
        command_id,
        private_generation,
        deadline,
        cancelled,
    )?
    .map(|(metadata, _, _)| metadata))
}

fn read_private_command_receipt_record(
    client: &mut impl GenericClient,
    scope: &PrivateOwnerScopeV1,
    target_ref: &str,
    command_id: &str,
    private_generation: u64,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> DurableResult<Option<(PrivateOwnerCommandReceiptMetadataV1, Vec<u8>, u64)>> {
    check_private_history_deadline(deadline, cancelled)?;
    let receipt_row = client.query_opt(
        "SELECT target_digest,family,request_digest,raw_request_digest,
                owner_context_binding,owner_configuration_digest,package_digest,
                response_bytes,commit_seq,private_audit_generation_after
         FROM cmd2_private_command_receipt
         WHERE domain=$1 AND owner_scope=$2 AND command_id=$3",
        &[&scope.domain, &scope.owner_scope, &command_id],
    )?;
    check_private_history_deadline(deadline, cancelled)?;
    let history_row = client.query_opt(
        "SELECT target_ref,family,package_digest,request_digest,raw_request_digest,
                owner_context_binding,owner_configuration_digest,
                private_owner_audit_generation,commit_seq,revision
         FROM cmd2_private_package_history
         WHERE domain=$1 AND owner_scope=$2 AND command_id=$3",
        &[&scope.domain, &scope.owner_scope, &command_id],
    )?;
    check_private_history_deadline(deadline, cancelled)?;
    let receipt = match (receipt_row, history_row) {
        (None, None) => None,
        (Some(_), None) | (None, Some(_)) => {
            return Err(DurableError::Corrupt(
                "private command receipt/history cardinality differs",
            ));
        }
        (Some(receipt), Some(history)) => {
            let response_bytes: Vec<u8> = receipt.get(7);
            let metadata = PrivateOwnerCommandReceiptMetadataV1 {
                command_id: command_id.to_owned(),
                target_digest: parse_hex(receipt.get(0))?,
                target_matches: false,
                family: parse_family(receipt.get(1))?,
                request_digest: parse_hex(receipt.get(2))?,
                raw_request_digest: parse_hex(receipt.get(3))?,
                owner_context_binding: parse_hex(receipt.get(4))?,
                owner_configuration_digest: parse_hex(receipt.get(5))?,
                package_digest: parse_hex(receipt.get(6))?,
                response_digest: Digest256::of_bytes(&response_bytes),
                commit_seq: as_u64(receipt.get(8))?,
                private_audit_generation_after: as_u64(receipt.get(9))?,
                source_private_generation: as_u64(history.get(7))?,
            };
            let history_target: String = history.get(0);
            let history_family = parse_family(history.get(1))?;
            let history_digest = parse_hex(history.get(2))?;
            let history_request = parse_hex(history.get(3))?;
            let history_raw_request = parse_hex(history.get(4))?;
            let history_context = parse_hex(history.get(5))?;
            let history_configuration = parse_hex(history.get(6))?;
            let history_commit_seq = as_u64(history.get(8))?;
            if response_bytes.len() > MAX_RECEIPT_BYTES
                || Digest256::of_bytes(history_target.as_bytes()) != metadata.target_digest
                || history_family != metadata.family
                || history_digest != metadata.package_digest
                || history_request != metadata.request_digest
                || history_raw_request != metadata.raw_request_digest
                || history_context != metadata.owner_context_binding
                || history_configuration != metadata.owner_configuration_digest
                || history_commit_seq != metadata.commit_seq
                || metadata.owner_context_binding != scope.context_binding
                || metadata.source_private_generation >= metadata.private_audit_generation_after
                || metadata.private_audit_generation_after > private_generation
            {
                return Err(DurableError::Corrupt(
                    "private command receipt/history binding",
                ));
            }
            let revision = as_u64(history.get(9))?;
            if revision == 0 {
                return Err(DurableError::Corrupt("private command history revision"));
            }
            Some((
                PrivateOwnerCommandReceiptMetadataV1 {
                    target_matches: Digest256::of_bytes(history_target.as_bytes())
                        == Digest256::of_bytes(target_ref.as_bytes()),
                    ..metadata
                },
                response_bytes,
                revision,
            ))
        }
    };
    check_private_history_deadline(deadline, cancelled)?;
    Ok(receipt)
}

fn validate_private_text_identity_lookup(
    scope: &PrivateOwnerScopeV1,
    tokens: &BTreeSet<String>,
    exclude_target: Option<&str>,
    allowed_ancestry: &BTreeMap<String, Digest256>,
) -> DurableResult<()> {
    validate_scope(scope)?;
    if tokens.len() > MAX_IDENTITY_ROWS
        || tokens
            .iter()
            .any(|token| token.is_empty() || token.len() > 4096 || token.contains('\0'))
        || allowed_ancestry.len() > MAX_IDENTITY_FILES
    {
        return Err(DurableError::Invalid("private Text identity lookup budget"));
    }
    if let Some(target_ref) = exclude_target {
        validate_target_ref(target_ref)?;
    }
    for (path, _) in allowed_ancestry {
        if path.is_empty()
            || path.len() > MAX_TARGET_REF_BYTES
            || path.contains('\0')
            || tos_foundation::RelativePath::parse(path).is_err()
            || !path.starts_with("ToS/")
        {
            return Err(DurableError::Invalid("private Text identity ancestry path"));
        }
    }
    Ok(())
}

fn read_private_text_identity_matches_from_client(
    client: &mut impl GenericClient,
    scope: &PrivateOwnerScopeV1,
    tokens: &BTreeSet<String>,
    exclude_target: Option<&str>,
    allowed_ancestry: &BTreeMap<String, Digest256>,
    private_generation: u64,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> DurableResult<PrivateTextIdentityLookupV1> {
    check_private_identity_deadline(deadline, cancelled)?;
    let mut matches = Vec::new();
    if !tokens.is_empty() {
        let token_values = tokens.iter().cloned().collect::<Vec<_>>();
        let exclude_value = exclude_target.map(str::to_owned);
        let limit = i64::try_from(MAX_IDENTITY_ROWS + 1)
            .map_err(|_| DurableError::Invalid("private Text identity lookup budget"))?;
        let rows = client.query(
            "SELECT i.identity_token,i.target_ref,i.member_name,i.member_digest,
                    i.revision,i.package_digest
             FROM cmd2_private_text_identity i
             JOIN cmd2_private_package_current c
               ON c.domain=i.domain AND c.owner_scope=i.owner_scope
              AND c.target_ref COLLATE \"C\"=i.target_ref AND c.family=i.family
              AND c.family IN ('text','alignment')
              AND c.revision=i.revision AND c.package_digest=i.package_digest
              AND c.text_identity_root=i.identity_root
             WHERE i.domain=$1 AND i.owner_scope=$2
               AND i.identity_token=ANY($3::text[])
               AND ($4::text IS NULL OR i.target_ref <> $4::text COLLATE \"C\")
             ORDER BY i.identity_token COLLATE \"C\",i.target_ref COLLATE \"C\",
                      i.member_name COLLATE \"C\" LIMIT $5",
            &[
                &scope.domain,
                &scope.owner_scope,
                &token_values,
                &exclude_value,
                &limit,
            ],
        )?;
        check_private_identity_deadline(deadline, cancelled)?;
        if rows.len() > MAX_IDENTITY_ROWS {
            return Err(DurableError::Refused(
                "private Text identity owner budget exceeded",
            ));
        }
        let mut seen = BTreeSet::new();
        for row in rows {
            check_private_identity_deadline(deadline, cancelled)?;
            let token: String = row.get(0);
            let target_ref: String = row.get(1);
            let member_name: String = row.get(2);
            let member_digest = parse_hex(row.get(3))?;
            let revision = as_u64(row.get(4))?;
            let package_digest = parse_hex(row.get(5))?;
            let path = format!("{target_ref}/{member_name}");
            if path.len() > MAX_TARGET_REF_BYTES
                || tos_foundation::RelativePath::parse(&path).is_err()
                || !path.starts_with("ToS/")
            {
                return Err(DurableError::Corrupt(
                    "private Text identity logical path budget",
                ));
            }
            if allowed_ancestry.get(&path) == Some(&member_digest) {
                continue;
            }
            if !seen.insert((token.clone(), path)) {
                return Err(DurableError::Corrupt(
                    "duplicate private Text identity point owner",
                ));
            }
            matches.push(PrivateTextIdentityMatchV1 {
                token,
                target_ref,
                member_name,
                member_digest,
                revision,
                package_digest,
            });
        }
    }
    check_private_identity_deadline(deadline, cancelled)?;
    Ok(PrivateTextIdentityLookupV1 {
        matches,
        private_audit_generation: private_generation,
    })
}

fn read_current_snapshot(
    client: &mut impl GenericClient,
    scope: &PrivateOwnerScopeV1,
    target_ref: &str,
    private_generation: u64,
) -> DurableResult<Option<PrivateOwnerPackageSnapshotV1>> {
    let Some(row) = client.query_opt(
        "SELECT family,revision,package_digest,member_count,total_bytes,
                text_identity_root,text_identity_count
         FROM cmd2_private_package_current
         WHERE domain=$1 AND owner_scope=$2 AND target_ref=$3",
        &[&scope.domain, &scope.owner_scope, &target_ref],
    )?
    else {
        return Ok(None);
    };
    let family = parse_family(row.get::<_, String>(0))?;
    let revision = as_u64(row.get(1))?;
    let digest = parse_hex(row.get(2))?;
    let count: i32 = row.get(3);
    let total = as_u64(row.get(4))?;
    let stored_identity_root = row.get::<_, Option<String>>(5).map(parse_hex).transpose()?;
    let stored_identity_count = row
        .get::<_, Option<i32>>(6)
        .map(|value| {
            usize::try_from(value).map_err(|_| DurableError::Corrupt("private Text identity count"))
        })
        .transpose()?;
    let rows = client.query(
        "SELECT member_name,member_bytes,member_digest
         FROM cmd2_private_package_member
         WHERE domain=$1 AND owner_scope=$2 AND target_ref=$3 AND revision=$4
         ORDER BY member_name COLLATE \"C\" LIMIT 66",
        &[
            &scope.domain,
            &scope.owner_scope,
            &target_ref,
            &(revision as i64),
        ],
    )?;
    if count <= 0 || rows.len() != count as usize {
        return Err(DurableError::Corrupt("private package member count"));
    }
    let mut members = BTreeMap::new();
    let mut observed_total = 0u64;
    for row in rows {
        let name: String = row.get(0);
        let bytes: Vec<u8> = row.get(1);
        let member_digest: String = row.get(2);
        if Digest256::of_bytes(&bytes).to_hex() != member_digest {
            return Err(DurableError::Corrupt("private package member digest"));
        }
        observed_total = observed_total
            .checked_add(bytes.len() as u64)
            .ok_or(DurableError::Corrupt("private package byte count overflow"))?;
        members.insert(name, bytes);
    }
    let (_, bounded_total) = validate_members(family, &members, false)?;
    if observed_total != total || bounded_total != total || package_digest(&members)? != digest {
        return Err(DurableError::Corrupt("private package digest or size"));
    }
    let (text_identity_root, text_identity_count) = validate_private_text_identity_snapshot(
        family,
        target_ref,
        &members,
        stored_identity_root,
        stored_identity_count,
    )?;
    Ok(Some(PrivateOwnerPackageSnapshotV1 {
        scope: scope.clone(),
        target_ref: target_ref.to_owned(),
        family,
        revision,
        package_digest: digest,
        members,
        text_identity_root,
        text_identity_count,
        private_audit_generation: private_generation,
    }))
}

fn read_current_text_identity_contributions(
    client: &mut impl GenericClient,
    scope: &PrivateOwnerScopeV1,
    target_ref: &str,
    family: PrivateOwnerPackageFamilyV1,
    revision: u64,
) -> DurableResult<Vec<PrivateTextIdentityContribution>> {
    let limit = i64::try_from(MAX_IDENTITY_ROWS + 1)
        .map_err(|_| DurableError::Invalid("private Text identity row budget"))?;
    let rows = client.query(
        "SELECT identity_token,member_name,member_digest
         FROM cmd2_private_text_identity
         WHERE domain=$1 AND owner_scope=$2 AND target_ref=$3 AND family=$4 AND revision=$5
         ORDER BY identity_token COLLATE \"C\",member_name COLLATE \"C\" LIMIT $6",
        &[
            &scope.domain,
            &scope.owner_scope,
            &target_ref,
            &family.as_str(),
            &as_i64(revision)?,
            &limit,
        ],
    )?;
    if rows.len() > MAX_IDENTITY_ROWS {
        return Err(DurableError::Corrupt(
            "private Text identity contribution row budget",
        ));
    }
    rows.into_iter()
        .map(|row| {
            let token: String = row.get(0);
            let member_name: String = row.get(1);
            if token.is_empty() || token.len() > 4096 || member_name.is_empty() {
                return Err(DurableError::Corrupt(
                    "private Text identity contribution shape",
                ));
            }
            Ok(PrivateTextIdentityContribution {
                token,
                member_name,
                member_digest: parse_hex(row.get(2))?,
            })
        })
        .collect()
}

fn parse_family(value: String) -> DurableResult<PrivateOwnerPackageFamilyV1> {
    match value.as_str() {
        "text" => Ok(PrivateOwnerPackageFamilyV1::Text),
        "alignment" => Ok(PrivateOwnerPackageFamilyV1::Alignment),
        "profile" => Ok(PrivateOwnerPackageFamilyV1::Profile),
        "claim" => Ok(PrivateOwnerPackageFamilyV1::Claim),
        "assessment" => Ok(PrivateOwnerPackageFamilyV1::Assessment),
        _ => Err(DurableError::Corrupt("private package family")),
    }
}

fn history_source_binding(row: &postgres::Row) -> DurableResult<PrivateOwnerSourceBindingV1> {
    Ok(PrivateOwnerSourceBindingV1 {
        head_seq: as_u64(row.get("source_head_seq"))?,
        source_epoch: as_u64(row.get("source_epoch"))?,
        source_generation: as_u64(row.get("source_generation"))?,
        source_revision: parse_hex(row.get("source_revision"))?,
        source_audit_generation: as_u64(row.get("source_audit_generation"))?,
        rights_version: as_u64(row.get("rights_version"))?,
        rule_version: as_u64(row.get("rule_version"))?,
        contract_digest: parse_hex(row.get("contract_digest"))?,
        job_id: row.get("job_id"),
        job_fence: as_u64(row.get("job_fence"))?,
        readset_digest: parse_hex(row.get("readset_digest"))?,
        dependency_digest: parse_hex(row.get("dependency_digest"))?,
        private_owner_audit_generation: as_u64(row.get("private_owner_audit_generation"))?,
    })
}

fn check_private_history_deadline(deadline: Instant, cancelled: &AtomicBool) -> DurableResult<()> {
    if cancelled.load(Ordering::Acquire) || Instant::now() >= deadline {
        Err(DurableError::Refused(
            "private package history deadline exceeded",
        ))
    } else {
        Ok(())
    }
}

fn check_private_identity_deadline(deadline: Instant, cancelled: &AtomicBool) -> DurableResult<()> {
    if cancelled.load(Ordering::Acquire) || Instant::now() >= deadline {
        Err(DurableError::Refused(
            "private Text identity lookup deadline exceeded",
        ))
    } else {
        Ok(())
    }
}

fn read_package_history(
    client: &mut impl GenericClient,
    scope: &PrivateOwnerScopeV1,
    target_ref: &str,
    max_rows: usize,
    max_total_bytes: usize,
    private_generation: u64,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> DurableResult<PrivateOwnerPackageHistoryV1> {
    check_private_history_deadline(deadline, cancelled)?;
    let rows = client.query(
        "SELECT revision,family,package_digest,previous_digest,member_count,total_bytes,
                command_id,request_digest,raw_request_digest,owner_context_binding,
                owner_configuration_digest,source_head_seq,source_epoch,source_generation,
                source_revision,source_audit_generation,private_owner_audit_generation,
                rights_version,rule_version,contract_digest,job_id,job_fence,readset_digest,
                dependency_digest,commit_seq
         FROM cmd2_private_package_history
         WHERE domain=$1 AND owner_scope=$2 AND target_ref=$3
         ORDER BY revision LIMIT $4",
        &[
            &scope.domain,
            &scope.owner_scope,
            &target_ref,
            &i64::try_from(max_rows + 1)
                .map_err(|_| DurableError::Invalid("private history row budget"))?,
        ],
    )?;
    check_private_history_deadline(deadline, cancelled)?;
    if rows.len() > max_rows {
        return Err(DurableError::Refused(
            "private owner package history row budget exceeded",
        ));
    }
    let mut history = Vec::with_capacity(rows.len());
    let mut total_bytes = 0usize;
    let mut previous_digest = None;
    let mut previous_revision = 0u64;
    let mut previous_commit_seq = 0u64;
    let mut previous_private_generation_after = 0u64;
    let mut family = None;
    let target_digest = Digest256::of_bytes(target_ref.as_bytes()).to_hex();
    for row in rows {
        check_private_history_deadline(deadline, cancelled)?;
        let revision = as_u64(row.get("revision"))?;
        let row_family = parse_family(row.get("family"))?;
        let package_hash = parse_hex(row.get("package_digest"))?;
        let previous = row
            .get::<_, Option<String>>("previous_digest")
            .map(parse_hex)
            .transpose()?;
        let count: i32 = row.get("member_count");
        let declared_total = as_u64(row.get("total_bytes"))?;
        let source_binding = history_source_binding(&row)?;
        let command_id: String = row.get("command_id");
        let request_digest = parse_hex(row.get("request_digest"))?;
        let raw_request_digest = parse_hex(row.get("raw_request_digest"))?;
        let owner_configuration_digest = parse_hex(row.get("owner_configuration_digest"))?;
        if revision != previous_revision.saturating_add(1)
            || previous != previous_digest
            || revision == 1 && previous.is_some()
            || revision > 1 && previous.is_none()
            || row.get::<_, String>("owner_context_binding") != scope.context_binding.to_hex()
            || family.is_some_and(|old| old != row_family)
        {
            return Err(DurableError::Corrupt(
                "private package history revision chain",
            ));
        }
        let member_rows = client.query(
            "SELECT member_name,member_bytes,member_digest
             FROM cmd2_private_package_member
             WHERE domain=$1 AND owner_scope=$2 AND target_ref=$3 AND revision=$4
             ORDER BY member_name COLLATE \"C\" LIMIT 66",
            &[
                &scope.domain,
                &scope.owner_scope,
                &target_ref,
                &as_i64(revision)?,
            ],
        )?;
        check_private_history_deadline(deadline, cancelled)?;
        if count <= 0 || member_rows.len() != count as usize {
            return Err(DurableError::Corrupt(
                "private package history member count",
            ));
        }
        let mut members = BTreeMap::new();
        let mut member_digest_bindings = BTreeMap::new();
        let mut observed_total = 0usize;
        for member in member_rows {
            check_private_history_deadline(deadline, cancelled)?;
            let name: String = member.get(0);
            let bytes: Vec<u8> = member.get(1);
            let digest: String = member.get(2);
            if Digest256::of_bytes(&bytes).to_hex() != digest
                || members.insert(name.clone(), bytes.clone()).is_some()
                || member_digest_bindings.insert(name, digest).is_some()
            {
                return Err(DurableError::Corrupt(
                    "private package history member digest",
                ));
            }
            observed_total = observed_total
                .checked_add(bytes.len())
                .ok_or(DurableError::Corrupt("private history byte count overflow"))?;
        }
        let (_, bounded_total) = validate_members(row_family, &members, false)?;
        let digest_from_members = package_digest_from_member_digests(&member_digest_bindings)?;
        if digest_from_members != package_hash
            || package_digest(&members)? != package_hash
            || observed_total as u64 != declared_total
            || bounded_total != declared_total
        {
            return Err(DurableError::Corrupt("private package history digest/size"));
        }
        let receipt = client
            .query_opt(
                "SELECT target_digest,family,request_digest,raw_request_digest,
                    owner_context_binding,owner_configuration_digest,package_digest,
                    response_bytes,commit_seq,private_audit_generation_after
             FROM cmd2_private_command_receipt
             WHERE domain=$1 AND owner_scope=$2 AND command_id=$3",
                &[&scope.domain, &scope.owner_scope, &command_id],
            )?
            .ok_or(DurableError::Corrupt(
                "private package history receipt absent",
            ))?;
        check_private_history_deadline(deadline, cancelled)?;
        let response_bytes: Vec<u8> = receipt.get("response_bytes");
        let row_bytes = observed_total
            .checked_add(response_bytes.len())
            .ok_or(DurableError::Corrupt("private history byte count overflow"))?;
        total_bytes = total_bytes
            .checked_add(row_bytes)
            .filter(|value| *value <= max_total_bytes)
            .ok_or(DurableError::Refused(
                "private owner package history byte budget exceeded",
            ))?;
        let commit_seq = as_u64(row.get("commit_seq"))?;
        let private_audit_generation_after = as_u64(receipt.get("private_audit_generation_after"))?;
        if receipt.get::<_, String>(0) != target_digest
            || receipt.get::<_, String>(1) != row_family.as_str()
            || receipt.get::<_, String>(2) != request_digest.to_hex()
            || receipt.get::<_, String>(3) != raw_request_digest.to_hex()
            || receipt.get::<_, String>(4) != scope.context_binding.to_hex()
            || receipt.get::<_, String>(5) != owner_configuration_digest.to_hex()
            || receipt.get::<_, String>(6) != package_hash.to_hex()
            || as_u64(receipt.get(8))? != commit_seq
            || as_u64(row.get("commit_seq"))? <= previous_commit_seq
            || source_binding.private_owner_audit_generation < previous_private_generation_after
            || source_binding.private_owner_audit_generation >= private_audit_generation_after
            || private_audit_generation_after > private_generation
            || response_bytes.len() > MAX_RECEIPT_BYTES
        {
            return Err(DurableError::Corrupt(
                "private package history receipt binding",
            ));
        }
        history.push(PrivateOwnerPackageHistoryEntryV1 {
            revision,
            family: row_family,
            package_digest: package_hash,
            previous_digest: previous,
            members,
            command_id,
            request_digest,
            raw_request_digest,
            owner_configuration_digest,
            source_binding,
            commit_seq,
            private_audit_generation_after,
            response_bytes,
        });
        previous_revision = revision;
        previous_digest = Some(package_hash);
        previous_commit_seq = commit_seq;
        previous_private_generation_after = private_audit_generation_after;
        family = Some(row_family);
    }
    let current_row = client.query_opt(
        "SELECT family,revision,package_digest,member_count,total_bytes,command_id,commit_seq,
                text_identity_root,text_identity_count
         FROM cmd2_private_package_current
         WHERE domain=$1 AND owner_scope=$2 AND target_ref=$3",
        &[&scope.domain, &scope.owner_scope, &target_ref],
    )?;
    check_private_history_deadline(deadline, cancelled)?;
    let current = match (history.last(), current_row) {
        (None, None) => None,
        (Some(last), Some(row)) => {
            let current_family = parse_family(row.get("family"))?;
            let stored_identity_root = row
                .get::<_, Option<String>>("text_identity_root")
                .map(parse_hex)
                .transpose()?;
            let stored_identity_count = row
                .get::<_, Option<i32>>("text_identity_count")
                .map(|value| {
                    usize::try_from(value)
                        .map_err(|_| DurableError::Corrupt("private Text identity count"))
                })
                .transpose()?;
            let (text_identity_root, text_identity_count) =
                validate_private_text_identity_snapshot(
                    current_family,
                    target_ref,
                    &last.members,
                    stored_identity_root,
                    stored_identity_count,
                )?;
            if current_family != last.family
                || as_u64(row.get("revision"))? != last.revision
                || parse_hex(row.get("package_digest"))? != last.package_digest
                || row.get::<_, i32>("member_count") as usize != last.members.len()
                || as_u64(row.get("total_bytes"))?
                    != last
                        .members
                        .values()
                        .map(|bytes| bytes.len() as u64)
                        .sum::<u64>()
                || row.get::<_, String>("command_id") != last.command_id
                || as_u64(row.get("commit_seq"))? != last.commit_seq
            {
                return Err(DurableError::Corrupt(
                    "private package current pointer differs from history head",
                ));
            }
            Some(PrivateOwnerPackageSnapshotV1 {
                scope: scope.clone(),
                target_ref: target_ref.to_owned(),
                family: last.family,
                revision: last.revision,
                package_digest: last.package_digest,
                members: last.members.clone(),
                text_identity_root,
                text_identity_count,
                private_audit_generation: private_generation,
            })
        }
        _ => {
            return Err(DurableError::Corrupt(
                "private package current/history cardinality differs",
            ));
        }
    };
    Ok(PrivateOwnerPackageHistoryV1 {
        scope: scope.clone(),
        target_ref: target_ref.to_owned(),
        family,
        current,
        history,
        private_audit_generation: private_generation,
    })
}

fn read_archive_snapshot(
    client: &mut impl GenericClient,
    scope: &PrivateOwnerScopeV1,
    target_ref: &str,
    archive_ref: &str,
    private_generation: u64,
) -> DurableResult<Option<PrivateOwnerArchiveSnapshotV1>> {
    let Some(row) = client.query_opt(
        "SELECT family,source_revision,package_digest,member_count,total_bytes
         FROM cmd2_private_package_archive
         WHERE domain=$1 AND owner_scope=$2 AND target_ref=$3 AND archive_ref=$4",
        &[&scope.domain, &scope.owner_scope, &target_ref, &archive_ref],
    )?
    else {
        return Ok(None);
    };
    let family = parse_family(row.get::<_, String>(0))?;
    let source_revision = parse_hex(row.get(1))?;
    let digest = parse_hex(row.get(2))?;
    let count: i32 = row.get(3);
    let total = as_u64(row.get(4))?;
    let rows = client.query(
        "SELECT member_name,member_bytes,member_digest
         FROM cmd2_private_package_archive_member
         WHERE domain=$1 AND owner_scope=$2 AND target_ref=$3 AND archive_ref=$4
         ORDER BY member_name COLLATE \"C\" LIMIT 66",
        &[&scope.domain, &scope.owner_scope, &target_ref, &archive_ref],
    )?;
    if count <= 0 || rows.len() != count as usize {
        return Err(DurableError::Corrupt(
            "private package archive member count",
        ));
    }
    let mut members = BTreeMap::new();
    let mut observed_total = 0u64;
    for row in rows {
        let name: String = row.get(0);
        let bytes: Vec<u8> = row.get(1);
        let member_digest: String = row.get(2);
        if Digest256::of_bytes(&bytes).to_hex() != member_digest {
            return Err(DurableError::Corrupt("private archive member digest"));
        }
        observed_total = observed_total
            .checked_add(bytes.len() as u64)
            .ok_or(DurableError::Corrupt("private archive byte count overflow"))?;
        members.insert(name, bytes);
    }
    let (_, bounded_total) = validate_members(family, &members, true)?;
    if observed_total != total || bounded_total != total || package_digest(&members)? != digest {
        return Err(DurableError::Corrupt("private archive digest or size"));
    }
    Ok(Some(PrivateOwnerArchiveSnapshotV1 {
        scope: scope.clone(),
        target_ref: target_ref.to_owned(),
        reference: archive_ref.to_owned(),
        family,
        source_revision,
        package_digest: digest,
        members,
        private_audit_generation: private_generation,
    }))
}

impl<'connection> PrivateOwnerReadSession<'connection> {
    fn transaction_mut(&mut self) -> DurableResult<&mut postgres::Transaction<'connection>> {
        self.transaction.as_mut().ok_or(DurableError::Corrupt(
            "private owner read session is closed",
        ))
    }

    pub(crate) fn scope(&self) -> &PrivateOwnerScopeV1 {
        &self.scope
    }

    pub(crate) fn private_audit_generation(&self) -> u64 {
        self.private_audit_generation
    }

    pub(crate) fn current_header(
        &mut self,
        target_ref: &str,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> DurableResult<Option<PrivateOwnerPackageCurrentHeaderV1>> {
        validate_target_ref(target_ref)?;
        check_private_history_deadline(deadline, cancelled)?;
        if let Some(header) = self.current_headers.get(target_ref) {
            return Ok(header.clone());
        }
        let scope = self.scope.clone();
        let generation = self.private_audit_generation;
        let header = read_current_package_header(
            self.transaction_mut()?,
            &scope,
            target_ref,
            generation,
            deadline,
            cancelled,
        )?;
        self.current_headers
            .insert(target_ref.to_owned(), header.clone());
        Ok(header)
    }

    pub(crate) fn command_metadata(
        &mut self,
        target_ref: &str,
        command_id: &str,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> DurableResult<PrivateOwnerCommandMetadataV1> {
        validate_target_ref(target_ref)?;
        if command_id.is_empty() || command_id.len() > 1024 || command_id.contains('\0') {
            return Err(DurableError::Invalid("private package command ID"));
        }
        let current = self.current_header(target_ref, deadline, cancelled)?;
        let scope = self.scope.clone();
        let generation = self.private_audit_generation;
        let receipt = read_private_command_receipt_metadata(
            self.transaction_mut()?,
            &scope,
            target_ref,
            command_id,
            generation,
            deadline,
            cancelled,
        )?;
        Ok(PrivateOwnerCommandMetadataV1 {
            header: PrivateOwnerPackageHeaderV1 {
                scope,
                target_ref: target_ref.to_owned(),
                current,
                private_audit_generation: generation,
            },
            receipt,
        })
    }

    /// Read exactly one member from the selected current revision. A metadata
    /// manifest check binds its member digest to the package digest before any
    /// member bytes are returned.
    pub(crate) fn read_current_member(
        &mut self,
        target_ref: &str,
        member_name: &str,
        max_bytes: usize,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> DurableResult<Option<PrivateOwnerSelectedMemberV1>> {
        if member_name.is_empty()
            || member_name == "."
            || member_name == ".."
            || member_name.len() > MAX_MEMBER_NAME_BYTES
            || member_name.contains('/')
            || member_name.contains('\\')
            || member_name.contains('\0')
            || max_bytes == 0
        {
            return Err(DurableError::Invalid("private package member read budget"));
        }
        let Some(header) = self.current_header(target_ref, deadline, cancelled)? else {
            return Ok(None);
        };
        if !matches!(
            header.family,
            PrivateOwnerPackageFamilyV1::Text | PrivateOwnerPackageFamilyV1::Alignment
        ) {
            return Err(DurableError::Refused(
                "private native source member family is not Text or Alignment",
            ));
        }
        let max_bytes = max_bytes.min(private_family_member_max_bytes(header.family));
        let scope = self.scope.clone();
        let generation = self.private_audit_generation;
        check_private_history_deadline(deadline, cancelled)?;
        let member_metadata = self.transaction_mut()?.query_opt(
            "SELECT member_digest,octet_length(member_bytes)
             FROM cmd2_private_package_member
             WHERE domain=$1 AND owner_scope=$2 AND target_ref=$3 AND revision=$4
               AND member_name=$5",
            &[
                &scope.domain,
                &scope.owner_scope,
                &target_ref,
                &as_i64(header.revision)?,
                &member_name,
            ],
        )?;
        check_private_history_deadline(deadline, cancelled)?;
        let Some(member_metadata) = member_metadata else {
            return Ok(None);
        };
        let expected_member_digest = parse_hex(member_metadata.get::<_, String>(0))?;
        let declared_size: i32 = member_metadata.get(1);
        if declared_size < 0 {
            return Err(DurableError::Corrupt("private package member size"));
        }
        let declared_size = declared_size as usize;
        if declared_size > max_bytes {
            return Err(DurableError::Refused(
                "private package selected member byte budget exceeded",
            ));
        }
        check_private_history_deadline(deadline, cancelled)?;
        let row = self.transaction_mut()?.query_one(
            "SELECT member_bytes,member_digest
             FROM cmd2_private_package_member
             WHERE domain=$1 AND owner_scope=$2 AND target_ref=$3 AND revision=$4
               AND member_name=$5",
            &[
                &scope.domain,
                &scope.owner_scope,
                &target_ref,
                &as_i64(header.revision)?,
                &member_name,
            ],
        )?;
        let bytes: Vec<u8> = row.get(0);
        let stored_member_digest = parse_hex(row.get(1))?;
        if bytes.len() != declared_size
            || stored_member_digest != expected_member_digest
            || Digest256::of_bytes(&bytes) != expected_member_digest
        {
            return Err(DurableError::Corrupt(
                "private package selected member digest",
            ));
        }
        check_private_history_deadline(deadline, cancelled)?;
        Ok(Some(PrivateOwnerSelectedMemberV1 {
            target_ref: target_ref.to_owned(),
            member_name: member_name.to_owned(),
            family: header.family,
            revision: header.revision,
            package_digest: header.package_digest,
            member_digest: expected_member_digest,
            bytes,
            private_audit_generation: generation,
        }))
    }

    /// Read one member from an explicitly pinned immutable package revision.
    /// The complete member-digest manifest and receipt are checked first; only
    /// the selected member bytes are fetched. Callers can therefore resolve
    /// private metadata before current rights checks and defer content reads
    /// until after those checks.
    pub(crate) fn read_selected_member_revision(
        &mut self,
        target_ref: &str,
        revision: u64,
        package_digest: Digest256,
        command_id: &str,
        member_name: &str,
        max_bytes: usize,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> DurableResult<Option<PrivateOwnerSelectedMemberV1>> {
        validate_target_ref(target_ref)?;
        if revision == 0
            || command_id.is_empty()
            || command_id.len() > 1024
            || command_id.contains('\0')
            || member_name.is_empty()
            || member_name == "."
            || member_name == ".."
            || member_name.len() > MAX_MEMBER_NAME_BYTES
            || member_name.contains('/')
            || member_name.contains('\\')
            || member_name.contains('\0')
            || max_bytes == 0
        {
            return Err(DurableError::Invalid(
                "private selected package revision member budget",
            ));
        }
        let current = self
            .current_header(target_ref, deadline, cancelled)?
            .ok_or(DurableError::Refused(
                "private selected package revision is absent",
            ))?;
        if current.family != PrivateOwnerPackageFamilyV1::Text || revision > current.revision {
            return Err(DurableError::Refused(
                "private selected package revision is outside Text history",
            ));
        }
        let max_bytes = max_bytes.min(private_family_member_max_bytes(current.family));
        let scope = self.scope.clone();
        let generation = self.private_audit_generation;
        check_private_history_deadline(deadline, cancelled)?;
        let row = self.transaction_mut()?.query_opt(
            "SELECT family,package_digest,member_count,total_bytes,command_id,commit_seq,
                    owner_context_binding,owner_configuration_digest,request_digest,
                    raw_request_digest,private_owner_audit_generation
             FROM cmd2_private_package_history
             WHERE domain=$1 AND owner_scope=$2 AND target_ref=$3 AND revision=$4",
            &[
                &scope.domain,
                &scope.owner_scope,
                &target_ref,
                &as_i64(revision)?,
            ],
        )?;
        check_private_history_deadline(deadline, cancelled)?;
        let Some(row) = row else {
            return Ok(None);
        };
        let family = parse_family(row.get(0))?;
        let stored_digest = parse_hex(row.get(1))?;
        let member_count: i32 = row.get(2);
        let total_bytes = as_u64(row.get(3))?;
        let stored_command_id: String = row.get(4);
        let commit_seq = as_u64(row.get(5))?;
        let owner_context_binding = parse_hex(row.get(6))?;
        let owner_configuration_digest = parse_hex(row.get(7))?;
        let request_digest = parse_hex(row.get(8))?;
        let raw_request_digest = parse_hex(row.get(9))?;
        let source_private_generation = as_u64(row.get(10))?;
        if family != PrivateOwnerPackageFamilyV1::Text
            || stored_digest != package_digest
            || stored_command_id != command_id
            || !(1..=MAX_MEMBERS as i32).contains(&member_count)
            || total_bytes == 0
            || total_bytes > private_family_max_bytes(family) as u64
            || owner_context_binding != scope.context_binding
            || source_private_generation >= generation
        {
            return Err(DurableError::Conflict(
                "private selected package revision pin differs",
            ));
        }
        verify_current_member_manifest(
            self.transaction_mut()?,
            &scope,
            target_ref,
            revision,
            member_count as usize,
            total_bytes,
            stored_digest,
            family,
        )?;
        let receipt = read_private_command_receipt_metadata(
            self.transaction_mut()?,
            &scope,
            target_ref,
            command_id,
            generation,
            deadline,
            cancelled,
        )?
        .ok_or(DurableError::Corrupt(
            "private selected package receipt absent",
        ))?;
        if !receipt.target_matches
            || receipt.family != family
            || receipt.package_digest != stored_digest
            || receipt.commit_seq != commit_seq
            || receipt.owner_context_binding != owner_context_binding
            || receipt.owner_configuration_digest != owner_configuration_digest
            || receipt.request_digest != request_digest
            || receipt.raw_request_digest != raw_request_digest
            || receipt.source_private_generation != source_private_generation
        {
            return Err(DurableError::Corrupt(
                "private selected package receipt differs",
            ));
        }
        check_private_history_deadline(deadline, cancelled)?;
        let member = self.transaction_mut()?.query_opt(
            "SELECT member_bytes,member_digest
             FROM cmd2_private_package_member
             WHERE domain=$1 AND owner_scope=$2 AND target_ref=$3 AND revision=$4
               AND member_name=$5",
            &[
                &scope.domain,
                &scope.owner_scope,
                &target_ref,
                &as_i64(revision)?,
                &member_name,
            ],
        )?;
        check_private_history_deadline(deadline, cancelled)?;
        let Some(member) = member else {
            return Ok(None);
        };
        let bytes: Vec<u8> = member.get(0);
        let member_digest = parse_hex(member.get(1))?;
        if bytes.len() > max_bytes || Digest256::of_bytes(&bytes) != member_digest {
            return Err(DurableError::Refused(
                "private selected package member digest or budget differs",
            ));
        }
        check_private_history_deadline(deadline, cancelled)?;
        Ok(Some(PrivateOwnerSelectedMemberV1 {
            target_ref: target_ref.to_owned(),
            member_name: member_name.to_owned(),
            family,
            revision,
            package_digest: stored_digest,
            member_digest,
            bytes,
            private_audit_generation: generation,
        }))
    }

    /// Hydrate a complete current package only when the owner operation has
    /// explicitly reached the content-admission phase for this target.
    pub(crate) fn read_alignment_output_package(
        &mut self,
        target_ref: &str,
        max_total_bytes: usize,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> DurableResult<Option<PrivateOwnerPackageSnapshotV1>> {
        validate_target_ref(target_ref)?;
        if max_total_bytes == 0 || max_total_bytes > MAX_ARCHIVE_BYTES {
            return Err(DurableError::Invalid("private package read byte budget"));
        }
        let Some(header) = self.current_header(target_ref, deadline, cancelled)? else {
            return Ok(None);
        };
        if header.family != PrivateOwnerPackageFamilyV1::Alignment {
            return Err(DurableError::Refused(
                "private Alignment output target has another family",
            ));
        }
        if header.total_bytes > max_total_bytes as u64 {
            return Err(DurableError::Refused(
                "private package selected byte budget exceeded",
            ));
        }
        let scope = self.scope.clone();
        let generation = self.private_audit_generation;
        check_private_history_deadline(deadline, cancelled)?;
        let snapshot =
            read_current_snapshot(self.transaction_mut()?, &scope, target_ref, generation)?.ok_or(
                DurableError::Corrupt("private package header disappeared in read snapshot"),
            )?;
        if snapshot.family != header.family
            || snapshot.revision != header.revision
            || snapshot.package_digest != header.package_digest
            || snapshot.members.len() != header.member_count
            || snapshot
                .members
                .values()
                .map(|bytes| bytes.len() as u64)
                .sum::<u64>()
                != header.total_bytes
        {
            return Err(DurableError::Corrupt(
                "private package body differs from selected header",
            ));
        }
        check_private_history_deadline(deadline, cancelled)?;
        Ok(Some(snapshot))
    }

    pub(crate) fn read_text_identity_matches(
        &mut self,
        tokens: &BTreeSet<String>,
        exclude_target: Option<&str>,
        allowed_ancestry: &BTreeMap<String, Digest256>,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> DurableResult<PrivateTextIdentityLookupV1> {
        validate_private_text_identity_lookup(
            &self.scope,
            tokens,
            exclude_target,
            allowed_ancestry,
        )?;
        let scope = self.scope.clone();
        let generation = self.private_audit_generation;
        read_private_text_identity_matches_from_client(
            self.transaction_mut()?,
            &scope,
            tokens,
            exclude_target,
            allowed_ancestry,
            generation,
            deadline,
            cancelled,
        )
    }
}

impl DurablePgCoordinator {
    /// Run an owner operation against one private RR snapshot while holding
    /// the owner-scope shared lock. The session lock is acquired before the
    /// RR snapshot. The callback may use another coordinator for public source
    /// reads, but must not recursively write this private scope. Package bytes
    /// are available only through exact session reads.
    pub(crate) fn with_private_owner_read_session<T>(
        &mut self,
        scope: &PrivateOwnerScopeV1,
        deadline: Instant,
        cancelled: &AtomicBool,
        callback: impl FnOnce(
            &mut PrivateOwnerReadSession<'_>,
            Instant,
            &AtomicBool,
        ) -> DurableResult<T>,
    ) -> DurableResult<T> {
        validate_scope(scope)?;
        check_private_history_deadline(deadline, cancelled)?;
        require_private_schema(&mut self.client)?;
        with_private_owner_shared_read_lock(
            &mut self.client,
            scope,
            deadline,
            cancelled,
            |client| {
                check_private_history_deadline(deadline, cancelled)?;
                let mut transaction = client.transaction()?;
                transaction.batch_execute("SET LOCAL lock_timeout='5s'; SET LOCAL statement_timeout='1s'; SET TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY")?;
                if !verify_context_binding(&mut transaction, scope)? {
                    return Err(DurableError::Refused(
                        "private owner context binding mismatch",
                    ));
                }
                let private_audit_generation = private_audit_generation(&mut transaction, scope)?;
                let mut session = PrivateOwnerReadSession {
                    transaction: Some(transaction),
                    scope: scope.clone(),
                    private_audit_generation,
                    current_headers: BTreeMap::new(),
                };
                let result = callback(&mut session, deadline, cancelled)?;
                check_private_history_deadline(deadline, cancelled)?;
                let transaction = session.transaction.take().ok_or(DurableError::Corrupt(
                    "private owner read session is closed",
                ))?;
                transaction.commit()?;
                Ok(result)
            },
        )
    }

    /// Resolve one exact historical receipt after the semantic owner has
    /// revalidated the command against current sources and authority. This
    /// reads no package members and never appends a command or revision.
    pub(crate) fn read_private_owner_command_replay(
        &mut self,
        scope: &PrivateOwnerScopeV1,
        target_ref: &str,
        family: PrivateOwnerPackageFamilyV1,
        command_id: &str,
        request_digest: Digest256,
        raw_request_digest: Digest256,
        owner_configuration_digest: Digest256,
        expected_private_generation: u64,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> DurableResult<PrivateOwnerPackageReplayV1> {
        validate_scope(scope)?;
        validate_target_ref(target_ref)?;
        if command_id.is_empty() || command_id.len() > 1024 || command_id.contains('\0') {
            return Err(DurableError::Invalid("private package command ID"));
        }
        check_private_history_deadline(deadline, cancelled)?;
        require_private_schema(&mut self.client)?;
        with_private_owner_shared_read_lock(
            &mut self.client,
            scope,
            deadline,
            cancelled,
            |client| {
                let mut tx = client.transaction()?;
                tx.batch_execute("SET LOCAL lock_timeout='5s'; SET LOCAL statement_timeout='1s'; SET TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY")?;
                if !verify_context_binding(&mut tx, scope)? {
                    return Err(DurableError::Refused(
                        "private owner context binding mismatch",
                    ));
                }
                let generation = private_audit_generation(&mut tx, scope)?;
                if generation != expected_private_generation {
                    return Err(DurableError::Conflict(
                        "private replay identity generation changed",
                    ));
                }
                let (receipt, response_bytes, package_revision) =
                    read_private_command_receipt_record(
                        &mut tx, scope, target_ref, command_id, generation, deadline, cancelled,
                    )?
                    .ok_or(DurableError::Corrupt(
                        "private replay durable receipt absent",
                    ))?;
                if !receipt.target_matches
                    || receipt.family != family
                    || receipt.request_digest != request_digest
                    || receipt.raw_request_digest != raw_request_digest
                    || receipt.owner_configuration_digest != owner_configuration_digest
                {
                    return Err(DurableError::Conflict(
                        "private replay command binding differs",
                    ));
                }
                check_private_history_deadline(deadline, cancelled)?;
                tx.commit()?;
                Ok(PrivateOwnerPackageReplayV1 {
                    replayed: true,
                    commit_seq: receipt.commit_seq,
                    package_revision,
                    package_digest: receipt.package_digest,
                    private_audit_generation_after: receipt.private_audit_generation_after,
                    response_bytes,
                })
            },
        )
    }

    pub(crate) fn read_private_owner_current(
        &mut self,
        scope: &PrivateOwnerScopeV1,
        target_ref: &str,
    ) -> DurableResult<Option<PrivateOwnerPackageSnapshotV1>> {
        validate_scope(scope)?;
        validate_target_ref(target_ref)?;
        require_private_schema(&mut self.client)?;
        let mut tx = self.client.transaction()?;
        tx.batch_execute("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY")?;
        if !verify_context_binding(&mut tx, scope)? {
            return Err(DurableError::Refused(
                "private owner context binding mismatch",
            ));
        }
        let generation = private_audit_generation(&mut tx, scope)?;
        let snapshot = read_current_snapshot(&mut tx, scope, target_ref, generation)?;
        tx.commit()?;
        Ok(snapshot)
    }

    pub(crate) fn read_private_owner_archive(
        &mut self,
        scope: &PrivateOwnerScopeV1,
        target_ref: &str,
        archive_ref: &str,
    ) -> DurableResult<Option<PrivateOwnerArchiveSnapshotV1>> {
        validate_scope(scope)?;
        validate_target_ref(target_ref)?;
        validate_target_ref(archive_ref)?;
        require_private_schema(&mut self.client)?;
        let mut tx = self.client.transaction()?;
        tx.batch_execute("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY")?;
        if !verify_context_binding(&mut tx, scope)? {
            return Err(DurableError::Refused(
                "private owner context binding mismatch",
            ));
        }
        let generation = private_audit_generation(&mut tx, scope)?;
        let snapshot = read_archive_snapshot(&mut tx, scope, target_ref, archive_ref, generation)?;
        tx.commit()?;
        Ok(snapshot)
    }


    /// Read one bounded set of immutable package histories under a single
    /// owner-scope lock and repeatable-read snapshot. The aggregate row and
    /// byte caps are applied while selecting each target, before the next
    /// target is accumulated. Callbacks must not recursively write this owner
    /// scope.
    pub(crate) fn with_private_owner_package_histories_guard<T>(
        &mut self,
        scope: &PrivateOwnerScopeV1,
        target_refs: &[String],
        max_total_rows: usize,
        max_total_bytes: usize,
        deadline: Instant,
        cancelled: &AtomicBool,
        callback: impl FnOnce(
            &BTreeMap<String, PrivateOwnerPackageHistoryV1>,
            Instant,
            &AtomicBool,
        ) -> DurableResult<T>,
    ) -> DurableResult<T> {
        validate_scope(scope)?;
        if target_refs.is_empty() || target_refs.len() > MAX_PACKAGE_HISTORY_TARGETS {
            return Err(DurableError::Invalid(
                "private package history target count",
            ));
        }
        let mut ordered_targets = BTreeSet::new();
        for target_ref in target_refs {
            validate_target_ref(target_ref)?;
            if !ordered_targets.insert(target_ref.as_str()) {
                return Err(DurableError::Invalid(
                    "duplicate private package history target",
                ));
            }
        }
        if max_total_rows == 0
            || max_total_rows > MAX_PACKAGE_HISTORY_ROWS
            || max_total_bytes == 0
            || max_total_bytes > MAX_PACKAGE_HISTORY_BYTES
        {
            return Err(DurableError::Invalid(
                "private package aggregate history budget",
            ));
        }
        check_private_history_deadline(deadline, cancelled)?;
        require_private_schema(&mut self.client)?;
        with_private_owner_shared_read_lock(
            &mut self.client,
            scope,
            deadline,
            cancelled,
            |client| {
                let mut tx = client.transaction()?;
                tx.batch_execute("SET LOCAL lock_timeout='5s'; SET LOCAL statement_timeout='1s'; SET TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY")?;
                if !verify_context_binding(&mut tx, scope)? {
                    return Err(DurableError::Refused(
                        "private owner context binding mismatch",
                    ));
                }
                let generation = private_audit_generation(&mut tx, scope)?;
                let mut histories = BTreeMap::new();
                let mut total_rows = 0usize;
                let mut total_bytes = 0usize;
                for target_ref in ordered_targets {
                    check_private_history_deadline(deadline, cancelled)?;
                    let remaining_rows = max_total_rows.saturating_sub(total_rows);
                    let remaining_bytes = max_total_bytes.saturating_sub(total_bytes);
                    let history = read_package_history(
                        &mut tx,
                        scope,
                        target_ref,
                        remaining_rows,
                        remaining_bytes,
                        generation,
                        deadline,
                        cancelled,
                    )?;
                    let observed_rows = history.history.len();
                    let observed_bytes = history
                        .history
                        .iter()
                        .try_fold(0usize, |total, entry| {
                            let members = entry
                                .members
                                .values()
                                .try_fold(0usize, |sum, bytes| sum.checked_add(bytes.len()))?;
                            total
                                .checked_add(members)?
                                .checked_add(entry.response_bytes.len())
                        })
                        .ok_or(DurableError::Refused(
                            "private owner aggregate history byte overflow",
                        ))?;
                    total_rows = total_rows
                        .checked_add(observed_rows)
                        .filter(|total| *total <= max_total_rows)
                        .ok_or(DurableError::Refused(
                            "private owner aggregate history row budget exceeded",
                        ))?;
                    total_bytes = total_bytes
                        .checked_add(observed_bytes)
                        .filter(|total| *total <= max_total_bytes)
                        .ok_or(DurableError::Refused(
                            "private owner aggregate history byte budget exceeded",
                        ))?;
                    histories.insert(target_ref.to_owned(), history);
                }
                check_private_history_deadline(deadline, cancelled)?;
                let result = callback(&histories, deadline, cancelled)?;
                check_private_history_deadline(deadline, cancelled)?;
                tx.commit()?;
                Ok(result)
            },
        )
    }

    /// Bounded current-only identity inventory. Archives and historical package
    /// versions are never considered identities. The result is tied to the
    /// private generation read in the same repeatable-read transaction.
    pub(crate) fn read_private_owner_identity_inputs(
        &mut self,
        scope: &PrivateOwnerScopeV1,
        exclude_target: Option<&str>,
        basenames: &BTreeSet<String>,
        include_provenance: bool,
    ) -> DurableResult<PrivateOwnerIdentityInputsV1> {
        validate_scope(scope)?;
        if let Some(target) = exclude_target {
            validate_target_ref(target)?;
        }
        if basenames.len() > 256 || basenames.iter().any(|v| v.is_empty() || v.len() > 128) {
            return Err(DurableError::Invalid("private identity basename budget"));
        }
        require_private_schema(&mut self.client)?;
        let mut tx = self.client.transaction()?;
        tx.batch_execute("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY")?;
        if !verify_context_binding(&mut tx, scope)? {
            return Err(DurableError::Refused(
                "private owner context binding mismatch",
            ));
        }
        let generation = private_audit_generation(&mut tx, scope)?;
        let mut package_rows = tx.query(
            "SELECT c.target_ref,c.family,c.revision,c.package_digest,c.member_count,c.total_bytes,
                    m.member_name,m.member_digest
             FROM cmd2_private_package_current c
             JOIN cmd2_private_package_member m
               ON m.domain=c.domain AND m.owner_scope=c.owner_scope
              AND m.target_ref=c.target_ref AND m.revision=c.revision
             WHERE c.domain=$1 AND c.owner_scope=$2
             ORDER BY c.target_ref COLLATE \"C\",m.member_name COLLATE \"C\"
             LIMIT $3",
            &[
                &scope.domain,
                &scope.owner_scope,
                &((MAX_IDENTITY_ROWS + 1) as i64),
            ],
        )?;
        if package_rows.len() > MAX_IDENTITY_ROWS {
            return Err(DurableError::Refused(
                "private identity entry budget exceeded",
            ));
        }
        #[derive(Default)]
        struct PackageMeta {
            family: Option<String>,
            revision: Option<i64>,
            package_digest: Option<String>,
            member_count: Option<i32>,
            total_bytes: Option<i64>,
            members: BTreeMap<String, String>,
        }
        let mut packages: BTreeMap<String, PackageMeta> = BTreeMap::new();
        for row in package_rows.drain(..) {
            let target: String = row.get(0);
            let family: String = row.get(1);
            let revision: i64 = row.get(2);
            let package_hash: String = row.get(3);
            let count: i32 = row.get(4);
            let total: i64 = row.get(5);
            let name: String = row.get(6);
            let digest: String = row.get(7);
            let family_max = match parse_family(family.clone())? {
                PrivateOwnerPackageFamilyV1::Text => MAX_TEXT_PACKAGE_BYTES,
                PrivateOwnerPackageFamilyV1::Alignment => MAX_ALIGNMENT_PACKAGE_BYTES,
                PrivateOwnerPackageFamilyV1::Profile | PrivateOwnerPackageFamilyV1::Claim => {
                    MAX_PROFILE_CLAIM_PACKAGE_BYTES
                }
                PrivateOwnerPackageFamilyV1::Assessment => MAX_ASSESSMENT_PACKAGE_BYTES,
            };
            let entry = packages.entry(target).or_default();
            if entry.family.as_ref().is_some_and(|old| old != &family)
                || entry.revision.is_some_and(|old| old != revision)
                || entry
                    .package_digest
                    .as_ref()
                    .is_some_and(|old| old != &package_hash)
                || entry.member_count.is_some_and(|old| old != count)
                || entry.total_bytes.is_some_and(|old| old != total)
                || total <= 0
                || total as usize > family_max
                || entry.members.insert(name, digest).is_some()
            {
                return Err(DurableError::Corrupt("private identity package metadata"));
            }
            entry.family = Some(family);
            entry.revision = Some(revision);
            entry.package_digest = Some(package_hash);
            entry.member_count = Some(count);
            entry.total_bytes = Some(total);
        }
        let mut selected = Vec::new();
        for (target, meta) in &packages {
            let path_components = target.split('/').collect::<Vec<_>>();
            let hidden_or_archive = path_components.iter().any(|part| {
                part.starts_with('.') || matches!(*part, "payload" | "local-content" | "catalog")
            });
            if hidden_or_archive || exclude_target == Some(target.as_str()) {
                continue;
            }
            let member_count = meta
                .member_count
                .ok_or(DurableError::Corrupt("private identity count"))?;
            if member_count <= 0 || meta.members.len() != member_count as usize {
                return Err(DurableError::Corrupt("private identity member count"));
            }
            let expected = meta
                .package_digest
                .as_ref()
                .ok_or(DurableError::Corrupt("private identity package digest"))?;
            if package_digest_from_member_digests(&meta.members)?.to_hex() != *expected {
                return Err(DurableError::Corrupt("private identity package digest"));
            }
            for name in meta.members.keys() {
                let provenance =
                    include_provenance && name.contains("provenance") && name.ends_with(".jsonl");
                if basenames.contains(name)
                    || name.ends_with(".human-forms.json")
                    || name.starts_with("semantic-annotation") && name.ends_with(".json")
                    || provenance
                {
                    selected.push((target.clone(), name.clone()));
                }
            }
        }
        if selected.len() > MAX_IDENTITY_FILES {
            return Err(DurableError::Refused(
                "private identity file budget exceeded",
            ));
        }
        let target_refs = selected.iter().map(|v| v.0.clone()).collect::<Vec<_>>();
        let member_names = selected.iter().map(|v| v.1.clone()).collect::<Vec<_>>();
        let rows = if selected.is_empty() {
            Vec::new()
        } else {
            tx.query(
                "SELECT c.target_ref,m.member_name,m.member_bytes,m.member_digest
                 FROM cmd2_private_package_current c
                 JOIN cmd2_private_package_member m
                   ON m.domain=c.domain AND m.owner_scope=c.owner_scope
                  AND m.target_ref=c.target_ref AND m.revision=c.revision
                 JOIN unnest($3::text[],$4::text[]) AS wanted(target_ref,member_name)
                   ON wanted.target_ref=c.target_ref AND wanted.member_name=m.member_name
                 WHERE c.domain=$1 AND c.owner_scope=$2
                 ORDER BY c.target_ref COLLATE \"C\",m.member_name COLLATE \"C\"",
                &[
                    &scope.domain,
                    &scope.owner_scope,
                    &target_refs,
                    &member_names,
                ],
            )?
        };
        if rows.len() != selected.len() {
            return Err(DurableError::Corrupt(
                "private identity selected member missing",
            ));
        }
        let mut files = BTreeMap::new();
        let mut total = 0usize;
        for row in rows {
            let target: String = row.get(0);
            let name: String = row.get(1);
            let bytes: Vec<u8> = row.get(2);
            let digest: String = row.get(3);
            if Digest256::of_bytes(&bytes).to_hex() != digest {
                return Err(DurableError::Corrupt("private identity member digest"));
            }
            total = total
                .checked_add(bytes.len())
                .ok_or(DurableError::Refused("private identity bytes overflow"))?;
            if total > MAX_IDENTITY_BYTES {
                return Err(DurableError::Refused(
                    "private identity byte budget exceeded",
                ));
            }
            let logical = format!("{target}/{name}");
            if files.insert(logical, bytes).is_some() {
                return Err(DurableError::Corrupt(
                    "private identity duplicate logical path",
                ));
            }
        }
        let visited_entries = packages.values().map(|v| v.members.len() + 1).sum();
        tx.commit()?;
        Ok(PrivateOwnerIdentityInputsV1 {
            files,
            visited_entries,
            private_audit_generation: generation,
        })
    }

    /// Resolve delegated private Text and Alignment tokens through the scoped
    /// multivalued point index. Alignment contributes only IDs parsed from
    /// translation-alignment JSON members; Text retains its existing broader
    /// candidate set. Exact target exclusion and exact path+digest ancestry
    /// admission mirror the public Text owner contract. The scalar generation
    /// binds empty and non-empty lookups to the same snapshot.
    pub(crate) fn read_private_text_identity_matches(
        &mut self,
        scope: &PrivateOwnerScopeV1,
        tokens: &BTreeSet<String>,
        exclude_target: Option<&str>,
        allowed_ancestry: &BTreeMap<String, Digest256>,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> DurableResult<PrivateTextIdentityLookupV1> {
        validate_private_text_identity_lookup(scope, tokens, exclude_target, allowed_ancestry)?;
        check_private_identity_deadline(deadline, cancelled)?;
        require_private_schema(&mut self.client)?;
        let mut tx = self.client.transaction()?;
        tx.batch_execute(
            "SET LOCAL statement_timeout='1s'; SET TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY",
        )?;
        if !verify_context_binding(&mut tx, scope)? {
            return Err(DurableError::Refused(
                "private owner context binding mismatch",
            ));
        }
        let generation = private_audit_generation(&mut tx, scope)?;
        let result = read_private_text_identity_matches_from_client(
            &mut tx,
            scope,
            tokens,
            exclude_target,
            allowed_ancestry,
            generation,
            deadline,
            cancelled,
        )?;
        tx.commit()?;
        Ok(result)
    }

    pub(crate) fn commit_private_owner_package(
        &mut self,
        capability: &mut impl PrivateOwnerPackageCapabilityV1,
    ) -> DurableResult<PrivateOwnerPackageReplayV1> {
        let write = capability.write().clone();
        validate_write(&write)?;
        require_private_schema(&mut self.client)?;
        let (after_digest, after_total) = validate_members(write.family, &write.members, false)?;
        let (text_identity_root, text_identity_rows) = if matches!(
            write.family,
            PrivateOwnerPackageFamilyV1::Text | PrivateOwnerPackageFamilyV1::Alignment
        ) {
            let (root, rows) = private_text_identity_contributions(
                write.family,
                &write.target_ref,
                &write.members,
            )?;
            (Some(root), rows)
        } else {
            (None, Vec::new())
        };
        let mut archive_shapes = Vec::with_capacity(write.archives.len());
        for archive in &write.archives {
            let (digest, total) = validate_members(write.family, &archive.members, true)?;
            archive_shapes.push((digest, total));
        }
        let mut tx = self.client.transaction()?;
        tx.batch_execute("SET LOCAL lock_timeout='5s'; SET LOCAL statement_timeout='15s'")?;
        let (lock_a, lock_b) = private_scope_advisory_keys(&write.scope);
        // Private history guards may need to run a public source registration
        // callback. Acquire this lock before any public source fence so a
        // waiting private writer never holds the fence that the callback needs.
        tx.query_one("SELECT pg_advisory_xact_lock($1,$2)", &[&lock_a, &lock_b])?;
        // Match public source writer lock order: audit fence, then domain, job.
        let fence = tx.query_opt(
            "SELECT generation,maintenance_state FROM cmd2_audit_fence WHERE domain=$1 FOR SHARE",
            &[&write.scope.domain],
        )?.ok_or(DurableError::Corrupt("public source audit fence absent"))?;
        let public_generation = as_u64(fence.get(0))?;
        if public_generation != write.source_binding.source_audit_generation
            || fence.get::<_, String>(1) != "normal"
        {
            return Err(DurableError::Conflict("public source audit fence changed"));
        }
        let domain = tx
            .query_opt(
                "SELECT head_seq,rights_version,rights_allowed,rule_version,contract_digest,
                    source_revision,source_epoch,source_generation
             FROM cmd2_domain WHERE domain=$1 FOR SHARE",
                &[&write.scope.domain],
            )?
            .ok_or(DurableError::Corrupt("public source domain absent"))?;
        let domain_revision: Option<String> = domain.get(5);
        let expected_source_revision = write.source_binding.source_revision.to_hex();
        if as_u64(domain.get(0))? != write.source_binding.head_seq
            || as_u64(domain.get(1))? != write.source_binding.rights_version
            || !domain.get::<_, bool>(2)
            || as_u64(domain.get(3))? != write.source_binding.rule_version
            || domain.get::<_, String>(4).trim() != write.source_binding.contract_digest.to_hex()
            || domain_revision.as_deref().map(str::trim) != Some(expected_source_revision.as_str())
            || domain.get::<_, Option<i64>>(6).map(as_u64).transpose()?
                != Some(write.source_binding.source_epoch)
            || domain.get::<_, Option<i64>>(7).map(as_u64).transpose()?
                != Some(write.source_binding.source_generation)
        {
            return Err(DurableError::Conflict("public source binding changed"));
        }
        let job = tx
            .query_opt(
                "SELECT fence_epoch FROM cmd2_job WHERE domain=$1 AND job_id=$2 FOR SHARE",
                &[&write.scope.domain, &write.source_binding.job_id],
            )?
            .ok_or(DurableError::Conflict("private owner job fence is absent"))?;
        if as_u64(job.get(0))? != write.source_binding.job_fence {
            return Err(DurableError::Conflict("private owner job fence changed"));
        }
        tx.execute(
            "INSERT INTO cmd2_private_owner_scope(domain,owner_scope,context_binding)
             VALUES($1,$2,$3) ON CONFLICT (domain,owner_scope) DO NOTHING",
            &[
                &write.scope.domain,
                &write.scope.owner_scope,
                &write.scope.context_binding.to_hex(),
            ],
        )?;
        tx.execute(
            "INSERT INTO cmd2_private_owner_audit_fence(domain,owner_scope,generation)
             VALUES($1,$2,0) ON CONFLICT (domain,owner_scope) DO NOTHING",
            &[&write.scope.domain, &write.scope.owner_scope],
        )?;
        let owner = tx.query_one(
            "SELECT context_binding,next_commit_seq FROM cmd2_private_owner_scope
             WHERE domain=$1 AND owner_scope=$2 FOR UPDATE",
            &[&write.scope.domain, &write.scope.owner_scope],
        )?;
        if owner.get::<_, String>(0) != write.scope.context_binding.to_hex() {
            return Err(DurableError::Refused("private owner scope context changed"));
        }
        let private_generation: i64 = tx
            .query_one(
                "SELECT generation FROM cmd2_private_owner_audit_fence
             WHERE domain=$1 AND owner_scope=$2 FOR UPDATE",
                &[&write.scope.domain, &write.scope.owner_scope],
            )?
            .get(0);
        let private_generation = as_u64(private_generation)?;
        let receipt_row = tx.query_opt(
            "SELECT target_digest,family,request_digest,raw_request_digest,owner_context_binding,
                    owner_configuration_digest,package_digest,response_bytes,commit_seq,
                    private_audit_generation_after,
                    (SELECT h.revision FROM cmd2_private_package_history h
                     WHERE h.domain=r.domain AND h.owner_scope=r.owner_scope
                       AND h.command_id=r.command_id
                       AND h.family=r.family AND h.commit_seq=r.commit_seq
                       AND h.package_digest=r.package_digest)
             FROM cmd2_private_command_receipt r
             WHERE domain=$1 AND owner_scope=$2 AND command_id=$3",
            &[
                &write.scope.domain,
                &write.scope.owner_scope,
                &write.command_id,
            ],
        )?;
        let replay = if let Some(row) = receipt_row {
            let replay = PrivateOwnerPackageReplayV1 {
                replayed: true,
                commit_seq: as_u64(row.get(8))?,
                package_revision: as_u64(row.get::<_, Option<i64>>(10).ok_or(
                    DurableError::Corrupt("private receipt package history absent"),
                )?)?,
                package_digest: parse_hex(row.get(6))?,
                private_audit_generation_after: as_u64(row.get(9))?,
                response_bytes: row.get(7),
            };
            let target_digest = Digest256::of_bytes(write.target_ref.as_bytes()).to_hex();
            if row.get::<_, String>(0) != target_digest
                || row.get::<_, String>(1) != write.family.as_str()
                || row.get::<_, String>(2) != write.request_digest.to_hex()
                || row.get::<_, String>(3) != write.raw_request_digest.to_hex()
                || row.get::<_, String>(4) != write.scope.context_binding.to_hex()
                || row.get::<_, String>(5) != write.owner_configuration_digest.to_hex()
                || private_generation < replay.private_audit_generation_after
                || private_generation < write.source_binding.private_owner_audit_generation
            {
                return Err(DurableError::Conflict(
                    "private command ID is already bound",
                ));
            }
            Some(replay)
        } else {
            if private_generation != write.source_binding.private_owner_audit_generation {
                return Err(DurableError::Conflict(
                    "private owner identity generation changed",
                ));
            }
            None
        };
        let current =
            read_current_snapshot(&mut tx, &write.scope, &write.target_ref, private_generation)?;
        if replay.is_none() && current != write.expected_current {
            return Err(DurableError::Conflict(
                "private package current snapshot changed",
            ));
        }
        if replay.is_none()
            && let Some(expected_heads) = capability.expected_history_heads()
        {
            if write.family != PrivateOwnerPackageFamilyV1::Assessment
                || expected_heads.is_empty()
                || expected_heads.len() > MAX_PACKAGE_HISTORY_TARGETS
            {
                return Err(DurableError::Invalid(
                    "private assessment expected history head closure",
                ));
            }
            for (target_ref, expected_head) in expected_heads {
                validate_target_ref(target_ref)?;
                let snapshot = if target_ref == &write.target_ref {
                    current.clone()
                } else {
                    read_current_snapshot(&mut tx, &write.scope, target_ref, private_generation)?
                };
                let actual_head = match snapshot {
                    Some(snapshot) => {
                        if snapshot.family != PrivateOwnerPackageFamilyV1::Assessment {
                            return Err(DurableError::Conflict(
                                "private assessment history target family changed",
                            ));
                        }
                        Some(
                            snapshot
                                .members
                                .get("head")
                                .ok_or(DurableError::Corrupt(
                                    "private assessment history head absent",
                                ))?
                                .clone(),
                        )
                    }
                    None => None,
                };
                if &actual_head != expected_head {
                    return Err(DurableError::Conflict(
                        "private assessment history closure changed",
                    ));
                }
            }
        }
        if replay.is_some() && current.as_ref().is_some_and(|p| p.family != write.family) {
            return Err(DurableError::Conflict("private package family changed"));
        }
        capability.revalidate(current.as_ref(), replay.as_ref())?;
        if let Some(replay) = replay {
            tx.commit()?;
            return Ok(replay);
        }
        if current.as_ref().is_some_and(|p| p.family != write.family) {
            return Err(DurableError::Conflict(
                "private package key belongs to another family",
            ));
        }
        let previous_text_identity_count = if let Some(current) =
            current.as_ref().filter(|snapshot| {
                matches!(
                    snapshot.family,
                    PrivateOwnerPackageFamilyV1::Text | PrivateOwnerPackageFamilyV1::Alignment
                )
            }) {
            let contributions = read_current_text_identity_contributions(
                &mut tx,
                &write.scope,
                &write.target_ref,
                current.family,
                current.revision,
            )?;
            let root = private_text_identity_root(&write.target_ref, &contributions)?;
            if Some(root) != current.text_identity_root
                || Some(contributions.len()) != current.text_identity_count
            {
                return Err(DurableError::Corrupt(
                    "private current Text identity rows differ from package marker",
                ));
            }
            let deleted = tx.execute(
                "DELETE FROM cmd2_private_text_identity
                 WHERE domain=$1 AND owner_scope=$2 AND target_ref=$3 AND family=$4 AND revision=$5",
                &[
                    &write.scope.domain,
                    &write.scope.owner_scope,
                    &write.target_ref,
                    &current.family.as_str(),
                    &as_i64(current.revision)?,
                ],
            )?;
            if usize::try_from(deleted).ok() != Some(contributions.len()) {
                return Err(DurableError::Corrupt(
                    "private current Text identity delete count",
                ));
            }
            contributions.len()
        } else {
            0
        };
        if !matches!(
            write.family,
            PrivateOwnerPackageFamilyV1::Text | PrivateOwnerPackageFamilyV1::Alignment
        ) && !text_identity_rows.is_empty()
        {
            return Err(DurableError::Corrupt(
                "non-Text private package produced identity rows",
            ));
        }
        let commit_seq = as_u64(owner.get(1))?
            .checked_add(1)
            .ok_or(DurableError::Invalid(
                "private owner commit sequence overflow",
            ))?;
        let commit_seq_i64 = as_i64(commit_seq)?;
        let revision = current
            .as_ref()
            .map(|snapshot| snapshot.revision.checked_add(1))
            .unwrap_or(Some(1))
            .ok_or(DurableError::Invalid("private package revision overflow"))?;
        let revision_i64 = as_i64(revision)?;
        let previous_digest = current.as_ref().map(|value| value.package_digest.to_hex());
        let generation_after = private_generation
            .checked_add(
                5 + write.members.len() as u64
                    + previous_text_identity_count as u64
                    + text_identity_rows.len() as u64
                    + write.archives.len() as u64
                    + write
                        .archives
                        .iter()
                        .map(|a| a.members.len() as u64)
                        .sum::<u64>(),
            )
            .ok_or(DurableError::Invalid(
                "private owner audit generation overflow",
            ))?;
        let source = &write.source_binding;
        tx.execute(
            "INSERT INTO cmd2_private_package_history(
               domain,owner_scope,target_ref,revision,family,package_digest,previous_digest,
               member_count,total_bytes,command_id,request_digest,raw_request_digest,
               owner_context_binding,owner_configuration_digest,source_head_seq,source_epoch,
               source_generation,source_revision,source_audit_generation,
               private_owner_audit_generation,rights_version,rule_version,
               contract_digest,job_id,job_fence,readset_digest,dependency_digest,commit_seq)
             VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16,$17,$18,$19,$20,
                    $21,$22,$23,$24,$25,$26,$27,$28)",
            &[
                &write.scope.domain,
                &write.scope.owner_scope,
                &write.target_ref,
                &revision_i64,
                &write.family.as_str(),
                &after_digest.to_hex(),
                &previous_digest,
                &(write.members.len() as i32),
                &(after_total as i64),
                &write.command_id,
                &write.request_digest.to_hex(),
                &write.raw_request_digest.to_hex(),
                &write.scope.context_binding.to_hex(),
                &write.owner_configuration_digest.to_hex(),
                &as_i64(source.head_seq)?,
                &as_i64(source.source_epoch)?,
                &as_i64(source.source_generation)?,
                &source.source_revision.to_hex(),
                &as_i64(source.source_audit_generation)?,
                &as_i64(source.private_owner_audit_generation)?,
                &as_i64(source.rights_version)?,
                &as_i64(source.rule_version)?,
                &source.contract_digest.to_hex(),
                &source.job_id,
                &as_i64(source.job_fence)?,
                &source.readset_digest.to_hex(),
                &source.dependency_digest.to_hex(),
                &commit_seq_i64,
            ],
        )?;
        for (name, bytes) in &write.members {
            tx.execute(
                "INSERT INTO cmd2_private_package_member(
                   domain,owner_scope,target_ref,revision,member_name,member_bytes,member_digest)
                 VALUES($1,$2,$3,$4,$5,$6,$7)",
                &[
                    &write.scope.domain,
                    &write.scope.owner_scope,
                    &write.target_ref,
                    &revision_i64,
                    name,
                    bytes,
                    &Digest256::of_bytes(bytes).to_hex(),
                ],
            )?;
        }
        for (archive, (digest, total)) in write.archives.iter().zip(archive_shapes) {
            tx.execute(
                "INSERT INTO cmd2_private_package_archive(
                   domain,owner_scope,target_ref,archive_ref,family,package_digest,member_count,
                   total_bytes,source_revision,command_id,commit_seq)
                 VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11)",
                &[
                    &write.scope.domain,
                    &write.scope.owner_scope,
                    &write.target_ref,
                    &archive.reference,
                    &write.family.as_str(),
                    &digest.to_hex(),
                    &(archive.members.len() as i32),
                    &(total as i64),
                    &archive.source_revision.to_hex(),
                    &write.command_id,
                    &commit_seq_i64,
                ],
            )?;
            for (name, bytes) in &archive.members {
                tx.execute(
                    "INSERT INTO cmd2_private_package_archive_member(
                       domain,owner_scope,target_ref,archive_ref,member_name,member_bytes,member_digest)
                     VALUES($1,$2,$3,$4,$5,$6,$7)",
                    &[&write.scope.domain, &write.scope.owner_scope, &write.target_ref,
                      &archive.reference, name, bytes, &Digest256::of_bytes(bytes).to_hex()],
                )?;
            }
        }
        tx.execute(
            "INSERT INTO cmd2_private_package_current(
               domain,owner_scope,target_ref,family,revision,package_digest,member_count,total_bytes,
               text_identity_root,text_identity_count,command_id,commit_seq)
             VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12)
             ON CONFLICT(domain,owner_scope,target_ref) DO UPDATE SET
               family=EXCLUDED.family,revision=EXCLUDED.revision,package_digest=EXCLUDED.package_digest,
               member_count=EXCLUDED.member_count,total_bytes=EXCLUDED.total_bytes,
               text_identity_root=EXCLUDED.text_identity_root,
               text_identity_count=EXCLUDED.text_identity_count,
               command_id=EXCLUDED.command_id,commit_seq=EXCLUDED.commit_seq",
            &[&write.scope.domain, &write.scope.owner_scope, &write.target_ref, &write.family.as_str(),
              &revision_i64, &after_digest.to_hex(), &(write.members.len() as i32),
              &(after_total as i64), &text_identity_root.map(Digest256::to_hex),
              &if matches!(write.family, PrivateOwnerPackageFamilyV1::Text | PrivateOwnerPackageFamilyV1::Alignment) {
                  Some(i32::try_from(text_identity_rows.len()).map_err(|_| DurableError::Invalid("private Text identity count"))?)
              } else {
                  None
              },
              &write.command_id, &commit_seq_i64],
        )?;
        for contribution in &text_identity_rows {
            tx.execute(
                "INSERT INTO cmd2_private_text_identity(
                   domain,owner_scope,target_ref,family,revision,package_digest,identity_root,
                   member_name,member_digest,identity_token)
                 VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10)",
                &[
                    &write.scope.domain,
                    &write.scope.owner_scope,
                    &write.target_ref,
                    &write.family.as_str(),
                    &revision_i64,
                    &after_digest.to_hex(),
                    &text_identity_root
                        .ok_or(DurableError::Corrupt("private Text identity root absent"))?
                        .to_hex(),
                    &contribution.member_name,
                    &contribution.member_digest.to_hex(),
                    &contribution.token,
                ],
            )?;
        }
        tx.execute(
            "INSERT INTO cmd2_private_command_receipt(
               domain,owner_scope,command_id,target_digest,family,request_digest,raw_request_digest,
               owner_context_binding,owner_configuration_digest,package_digest,response_bytes,commit_seq,
               private_audit_generation_after)
             VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13)",
            &[&write.scope.domain, &write.scope.owner_scope, &write.command_id,
              &Digest256::of_bytes(write.target_ref.as_bytes()).to_hex(), &write.family.as_str(),
              &write.request_digest.to_hex(), &write.raw_request_digest.to_hex(),
              &write.scope.context_binding.to_hex(), &write.owner_configuration_digest.to_hex(),
              &after_digest.to_hex(), &write.response_bytes, &commit_seq_i64,
              &as_i64(generation_after)?],
        )?;
        let target_digest = Digest256::of_bytes(write.target_ref.as_bytes()).to_hex();
        let command_digest = Digest256::of_bytes(write.command_id.as_bytes()).to_hex();
        tx.execute(
            "INSERT INTO cmd2_private_log(domain,owner_scope,commit_seq,family,command_digest,
               target_digest,request_digest,package_digest,member_count)
             VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9)",
            &[
                &write.scope.domain,
                &write.scope.owner_scope,
                &commit_seq_i64,
                &write.family.as_str(),
                &command_digest,
                &target_digest,
                &write.request_digest.to_hex(),
                &after_digest.to_hex(),
                &(write.members.len() as i32),
            ],
        )?;
        tx.execute(
            "INSERT INTO cmd2_private_outbox(domain,owner_scope,commit_seq,family,target_digest,
               package_digest,member_count)
             VALUES($1,$2,$3,$4,$5,$6,$7)",
            &[
                &write.scope.domain,
                &write.scope.owner_scope,
                &commit_seq_i64,
                &write.family.as_str(),
                &target_digest,
                &after_digest.to_hex(),
                &(write.members.len() as i32),
            ],
        )?;
        tx.execute(
            "UPDATE cmd2_private_owner_scope SET next_commit_seq=$3
             WHERE domain=$1 AND owner_scope=$2",
            &[
                &write.scope.domain,
                &write.scope.owner_scope,
                &commit_seq_i64,
            ],
        )?;
        let observed_generation: i64 = tx
            .query_one(
                "SELECT generation FROM cmd2_private_owner_audit_fence
             WHERE domain=$1 AND owner_scope=$2",
                &[&write.scope.domain, &write.scope.owner_scope],
            )?
            .get(0);
        if as_u64(observed_generation)? != generation_after {
            return Err(DurableError::Corrupt(
                "private audit generation trigger count",
            ));
        }
        let receipt = PrivateOwnerPackageReplayV1 {
            replayed: false,
            commit_seq,
            package_revision: revision,
            package_digest: after_digest,
            private_audit_generation_after: generation_after,
            response_bytes: write.response_bytes,
        };
        tx.commit()?;
        Ok(receipt)
    }
}

fn validate_write(write: &PrivateOwnerPackageWriteV1) -> DurableResult<()> {
    validate_scope(&write.scope)?;
    validate_target_ref(&write.target_ref)?;
    if write.command_id.is_empty()
        || write.command_id.len() > 1024
        || write.command_id.contains('\0')
        || write.response_bytes.len() > MAX_RECEIPT_BYTES
        || write.archives.len() > MAX_ARCHIVES_PER_COMMIT
    {
        return Err(DurableError::Invalid(
            "private package write identity/budget",
        ));
    }
    if let Some(expected) = &write.expected_current {
        if expected.scope != write.scope
            || expected.target_ref != write.target_ref
            || expected.family != write.family
        {
            return Err(DurableError::Invalid("private expected snapshot identity"));
        }
        validate_members(write.family, &expected.members, false)?;
        if package_digest(&expected.members)? != expected.package_digest {
            return Err(DurableError::Invalid("private expected snapshot digest"));
        }
        validate_private_text_identity_snapshot(
            expected.family,
            &expected.target_ref,
            &expected.members,
            expected.text_identity_root,
            expected.text_identity_count,
        )?;
    }
    let _ = validate_members(write.family, &write.members, false)?;
    let mut archive_refs = BTreeSet::new();
    for archive in &write.archives {
        validate_target_ref(&archive.reference)?;
        if !archive_refs.insert(archive.reference.as_str()) {
            return Err(DurableError::Invalid("duplicate private archive reference"));
        }
    }
    if write.source_binding.job_id.is_empty()
        || write.source_binding.job_id.len() > 256
        || write.source_binding.job_fence == 0
        || write.source_binding.source_epoch == 0
    {
        return Err(DurableError::Invalid("private source binding identity"));
    }
    Ok(())
}
